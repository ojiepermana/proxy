//! Pemilihan backend sehat secara round robin sesuai urutan konfigurasi.

mod health;
pub use health::{CompletedProbe, HealthCheckMode, HttpHealthOptions, ProbeResult};
use health::{CountedHealthCheck, ProbeStreaks};

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::FutureExt;
use pingora::lb::{Backend, Backends, HealthCheckService, HealthRegistry, discovery::Static};

pub struct BackendStatus {
    pub address: SocketAddr,
    pub healthy: bool,
    pub consecutive_successes: usize,
    pub consecutive_failures: usize,
    pub last_probe: Option<CompletedProbe>,
}

/// Urutan konfigurasi dan kesehatan backend yang dipakai bersama oleh semua request.
pub struct RoundRobinPool {
    targets: Box<[Backend]>,
    view: Arc<Backends>,
    registry: Arc<HealthRegistry>,
    streaks: Arc<ProbeStreaks>,
    health_check_interval: Duration,
    next: Mutex<usize>,
}

impl RoundRobinPool {
    pub fn new(first: SocketAddr, remaining: &[SocketAddr]) -> Self {
        Self::with_health_check(first, remaining, HealthCheckMode::Tcp)
    }

    pub fn with_health_check(
        first: SocketAddr,
        remaining: &[SocketAddr],
        mode: HealthCheckMode,
    ) -> Self {
        Self::with_health_check_options(first, remaining, mode, HttpHealthOptions::default())
            .expect("konfigurasi health check bawaan valid")
    }

    pub fn with_health_check_options(
        first: SocketAddr,
        remaining: &[SocketAddr],
        mode: HealthCheckMode,
        options: HttpHealthOptions,
    ) -> pingora::Result<Self> {
        let targets: Box<[_]> = std::iter::once(first)
            .chain(remaining.iter().copied())
            .map(|addr| Backend {
                addr: addr.into(),
                weight: 1,
                ext: Default::default(),
            })
            .collect();
        let registry = Arc::new(HealthRegistry::new());
        let discovery = Static::new(targets.iter().cloned().collect());
        let view = Arc::new(Backends::new_with_health_registry(
            discovery,
            Arc::clone(&registry),
        ));
        // Discovery statis hanya menyalin alamat dari memori, sebelum server dimulai.
        view.update(|_| {})
            .now_or_never()
            .expect("discovery statis selesai tanpa I/O")
            .expect("alamat backend statis sudah valid");
        let inner = mode.build(&view, &options)?;
        let streaks = Arc::new(ProbeStreaks::new(&view, options.start_unhealthy));
        registry.set_health_check(Box::new(CountedHealthCheck {
            mode,
            inner,
            streaks: Arc::clone(&streaks),
        }));
        Ok(Self {
            targets,
            view,
            registry,
            streaks,
            health_check_interval: Duration::from_millis(
                options.interval_ms.map_or(1000, |ms| ms.get().into()),
            ),
            next: Mutex::new(0),
        })
    }

    pub fn health_checks(&self) -> HealthCheckService {
        let mut checks = HealthCheckService::new(Arc::clone(&self.registry));
        checks.health_check_frequency = Some(self.health_check_interval);
        checks.parallel_health_check = true;
        checks
    }

    /// Alamat dan kesiapan tiap backend sesuai urutan konfigurasi, tanpa mengambil giliran.
    pub fn backend_statuses(&self) -> impl Iterator<Item = (SocketAddr, bool)> + '_ {
        self.backend_probe_statuses()
            .map(|status| (status.address, status.healthy))
    }

    /// Kesiapan, penghitung, dan probe terakhir yang selesai pada setiap alamat.
    pub fn backend_probe_statuses(&self) -> impl Iterator<Item = BackendStatus> + '_ {
        self.targets.iter().map(|target| {
            let state = self.streaks.for_target(target);
            BackendStatus {
                address: *target.addr.as_inet().expect("target berupa alamat IP"),
                healthy: self.view.ready(target),
                consecutive_successes: state.consecutive_successes,
                consecutive_failures: state.consecutive_failures,
                last_probe: state.last_probe,
            }
        })
    }

    pub fn select(&self) -> Option<SocketAddr> {
        self.select_excluding(&[])
    }

    /// Alamat yang sudah gagal dihubungi untuk request ini dilewati.
    pub fn select_excluding(&self, failed: &[SocketAddr]) -> Option<SocketAddr> {
        let mut next = self.next.lock().expect("kursor round robin");
        for _ in 0..self.targets.len() {
            let target = &self.targets[*next];
            *next = (*next + 1) % self.targets.len();
            let address = *target.addr.as_inet().expect("target berupa alamat IP");
            if self.view.ready(target) && !failed.contains(&address) {
                return Some(address);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[tokio::test]
    async fn selects_only_backends_that_pass_tcp_probes() {
        let a = TcpListener::bind("127.0.0.1:0").unwrap();
        let b = TcpListener::bind("127.0.0.1:0").unwrap();
        let c = TcpListener::bind("127.0.0.1:0").unwrap();
        let [a_addr, b_addr, c_addr] = [&a, &b, &c].map(|s| s.local_addr().unwrap());
        let pool = RoundRobinPool::new(a_addr, &[b_addr, c_addr]);
        let counts = || {
            pool.backend_probe_statuses()
                .map(|status| (status.consecutive_successes, status.consecutive_failures))
                .collect::<Vec<_>>()
        };
        assert_eq!(counts(), [(0, 0), (0, 0), (0, 0)]);

        drop(b);
        pool.registry.run_health_check(false).await;
        assert_eq!(counts(), [(1, 0), (0, 1), (1, 0)]);
        let results: Vec<_> = pool
            .backend_probe_statuses()
            .map(|status| status.last_probe.unwrap().result)
            .collect();
        assert_eq!(
            results,
            [
                ProbeResult::TcpConnected,
                ProbeResult::Error,
                ProbeResult::TcpConnected
            ]
        );
        for expected in [a_addr, c_addr, a_addr, c_addr] {
            assert_eq!(pool.select(), Some(expected));
        }

        let b = TcpListener::bind(b_addr).unwrap();
        pool.registry.run_health_check(false).await;
        assert_eq!(counts(), [(2, 0), (1, 0), (2, 0)]);
        for expected in [a_addr, b_addr, c_addr] {
            assert_eq!(pool.select(), Some(expected));
        }

        drop((a, b, c));
        pool.registry.run_health_check(false).await;
        assert_eq!(counts(), [(0, 1), (0, 1), (0, 1)]);
        assert_eq!(pool.select(), None);
    }
}
