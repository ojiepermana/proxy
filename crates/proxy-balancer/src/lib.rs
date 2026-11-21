//! Pemilihan backend sehat secara round robin sesuai urutan konfigurasi.

mod health;
pub use health::HealthCheckMode;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::FutureExt;
use pingora::lb::{Backend, Backends, HealthCheckService, HealthRegistry, discovery::Static};

/// Urutan konfigurasi dan kesehatan backend yang dipakai bersama oleh semua request.
pub struct RoundRobinPool {
    targets: Box<[Backend]>,
    view: Backends,
    registry: Arc<HealthRegistry>,
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
        let targets: Box<[_]> = std::iter::once(first)
            .chain(remaining.iter().copied())
            .map(|addr| Backend {
                addr: addr.into(),
                weight: 1,
                ext: Default::default(),
            })
            .collect();
        let registry = Arc::new(HealthRegistry::new());
        registry.set_health_check(mode.build(&targets));
        let discovery = Static::new(targets.iter().cloned().collect());
        let view = Backends::new_with_health_registry(discovery, Arc::clone(&registry));
        // Discovery statis hanya menyalin alamat dari memori, sebelum server dimulai.
        view.update(|_| {})
            .now_or_never()
            .expect("discovery statis selesai tanpa I/O")
            .expect("alamat backend statis sudah valid");
        Self {
            targets,
            view,
            registry,
            next: Mutex::new(0),
        }
    }

    pub fn health_checks(&self) -> HealthCheckService {
        let mut checks = HealthCheckService::new(Arc::clone(&self.registry));
        checks.health_check_frequency = Some(Duration::from_secs(1));
        checks.parallel_health_check = true;
        checks
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

        drop(b);
        pool.registry.run_health_check(false).await;
        for expected in [a_addr, c_addr, a_addr, c_addr] {
            assert_eq!(pool.select(), Some(expected));
        }

        let b = TcpListener::bind(b_addr).unwrap();
        pool.registry.run_health_check(false).await;
        for expected in [a_addr, b_addr, c_addr] {
            assert_eq!(pool.select(), Some(expected));
        }

        drop((a, b, c));
        pool.registry.run_health_check(false).await;
        assert_eq!(pool.select(), None);
    }
}
