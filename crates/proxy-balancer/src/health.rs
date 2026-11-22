use std::{
    collections::HashMap,
    num::{NonZeroU32, NonZeroUsize},
    str::FromStr,
    sync::{Arc, Mutex, MutexGuard, Weak},
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use pingora::lb::{
    Backend, Backends,
    health_check::{HealthCheck, HttpHealthCheck, TcpHealthCheck},
};
use pingora::{Error, ErrorType, Result, protocols::l4::socket::SocketAddr};

#[derive(Clone, Copy)]
pub enum HealthCheckMode {
    Tcp,
    Http,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeResult {
    HttpStatus(u16),
    TcpConnected,
    Timeout,
    Error,
}

#[derive(Clone, Copy, Debug)]
pub struct CompletedProbe {
    pub completed_at: SystemTime,
    pub result: ProbeResult,
}

/// Opsi eksplisit untuk probe HTTP. Nilai kosong mempertahankan pengaturan bawaan.
#[derive(Default, PartialEq)]
pub struct HttpHealthOptions {
    pub path: Option<String>,
    pub interval_ms: Option<NonZeroU32>,
    pub timeout_ms: Option<NonZeroU32>,
    pub failure_threshold: Option<NonZeroUsize>,
    pub success_threshold: Option<NonZeroUsize>,
    pub start_unhealthy: bool,
}

impl FromStr for HealthCheckMode {
    type Err = &'static str;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "tcp" => Ok(Self::Tcp),
            "http" => Ok(Self::Http),
            _ => Err("Gunakan tcp atau http"),
        }
    }
}

impl HealthCheckMode {
    fn probe_result(self, result: &Result<()>) -> ProbeResult {
        match result {
            Ok(()) => match self {
                Self::Tcp => ProbeResult::TcpConnected,
                Self::Http => ProbeResult::HttpStatus(200),
            },
            Err(error) => match error.etype() {
                // Validator bawaan HttpHealthCheck mengembalikan kode respons dalam varian ini.
                ErrorType::CustomCode("non 200 code", status) => ProbeResult::HttpStatus(*status),
                ErrorType::ConnectTimedout | ErrorType::ReadTimedout | ErrorType::WriteTimedout => {
                    ProbeResult::Timeout
                }
                _ => ProbeResult::Error,
            },
        }
    }

    pub(crate) fn build(
        self,
        view: &Arc<Backends>,
        options: &HttpHealthOptions,
    ) -> Result<Box<dyn HealthCheck + Send + Sync>> {
        match self {
            Self::Tcp if *options != HttpHealthOptions::default() => Error::e_explain(
                ErrorType::Custom("Konfigurasi health check"),
                "Opsi probe HTTP memerlukan --health-check http",
            ),
            Self::Tcp => Ok(TcpHealthCheck::new()),
            Self::Http => Ok(Box::new(HttpEndpointChecks::new(view, options)?)),
        }
    }
}

#[derive(Default)]
pub(super) struct ProbeState {
    pub(super) consecutive_successes: usize,
    pub(super) consecutive_failures: usize,
    pub(super) last_probe: Option<CompletedProbe>,
    startup: Option<Weak<Backends>>,
}

impl ProbeState {
    fn observe(&mut self, target: &Backend, probe: CompletedProbe, success_threshold: usize) {
        self.last_probe = Some(probe);
        if matches!(
            probe.result,
            ProbeResult::TcpConnected | ProbeResult::HttpStatus(200)
        ) {
            self.consecutive_successes = self.consecutive_successes.saturating_add(1);
            self.consecutive_failures = 0;
            if self.consecutive_successes == success_threshold
                && let Some(view) = self.startup.take().and_then(|view| view.upgrade())
            {
                view.set_enable(target, true);
            }
        } else {
            self.consecutive_failures = self.consecutive_failures.saturating_add(1);
            self.consecutive_successes = 0;
        }
    }
}

pub(super) struct ProbeStreaks(HashMap<SocketAddr, Mutex<ProbeState>>);

impl ProbeStreaks {
    pub(super) fn new(view: &Arc<Backends>, start_unhealthy: bool) -> Self {
        let mut states = HashMap::new();
        for target in view.get_backend().iter() {
            let mut state = ProbeState::default();
            if start_unhealthy {
                view.set_enable(target, false);
                state.startup = Some(Arc::downgrade(view));
            }
            states.insert(target.addr.clone(), Mutex::new(state));
        }
        Self(states)
    }

    pub(super) fn for_target(&self, target: &Backend) -> MutexGuard<'_, ProbeState> {
        self.0[&target.addr].lock().expect("penghitung probe")
    }
}

pub(super) struct CountedHealthCheck {
    pub(super) mode: HealthCheckMode,
    pub(super) inner: Box<dyn HealthCheck + Send + Sync>,
    pub(super) streaks: Arc<ProbeStreaks>,
}

#[async_trait]
impl HealthCheck for CountedHealthCheck {
    async fn check(&self, target: &Backend) -> Result<()> {
        let result = self.inner.check(target).await;
        let probe = CompletedProbe {
            completed_at: SystemTime::now(),
            result: self.mode.probe_result(&result),
        };
        self.streaks
            .for_target(target)
            .observe(target, probe, self.health_threshold(true));
        result
    }

    fn health_threshold(&self, success: bool) -> usize {
        self.inner.health_threshold(success)
    }

    async fn health_status_change(&self, target: &Backend, healthy: bool) {
        self.inner.health_status_change(target, healthy).await;
    }

    fn backend_summary(&self, target: &Backend) -> String {
        self.inner.backend_summary(target)
    }
}

// Setiap backend memiliki header Host sendiri. Probe dipakai ulang antarputaran.
struct HttpEndpointChecks {
    probes: HashMap<SocketAddr, HttpHealthCheck>,
    timeout: Duration,
    failure_threshold: usize,
    success_threshold: usize,
}

impl HttpEndpointChecks {
    fn new(view: &Arc<Backends>, options: &HttpHealthOptions) -> Result<Self> {
        let path = options.path.as_deref().unwrap_or("/health");
        if !path.starts_with('/') || path.contains('#') {
            return Error::e_explain(
                ErrorType::InvalidHTTPHeader,
                "--health-check-path harus diawali / dan bebas dari fragmen (#)",
            );
        }
        let timeout = options
            .timeout_ms
            .map(|ms| Duration::from_millis(ms.get().into()));
        let targets = view.get_backend();
        let mut probes = HashMap::with_capacity(targets.len());
        for target in targets.iter() {
            let mut probe = HttpHealthCheck::new(&target.addr.to_string(), false);
            probe.req.set_raw_path(path.as_bytes())?;
            if let Some(timeout) = timeout {
                probe.peer_template.options.connection_timeout = Some(timeout);
                probe.peer_template.options.read_timeout = Some(timeout);
                probe.peer_template.options.write_timeout = Some(timeout);
            }
            probes.insert(target.addr.clone(), probe);
        }
        Ok(Self {
            probes,
            timeout: timeout.unwrap_or(Duration::from_secs(2)),
            failure_threshold: options.failure_threshold.map_or(1, NonZeroUsize::get),
            success_threshold: options.success_threshold.map_or(1, NonZeroUsize::get),
        })
    }
}

#[async_trait]
impl HealthCheck for HttpEndpointChecks {
    async fn check(&self, target: &Backend) -> Result<()> {
        let probe = self.probes.get(&target.addr).ok_or_else(|| {
            Error::explain(ErrorType::InternalError, "Backend probe belum terdaftar")
        })?;
        tokio::time::timeout(self.timeout, probe.check(target))
            .await
            .unwrap_or_else(|e| {
                Err(Error::because(
                    ErrorType::ReadTimedout,
                    "Batas waktu probe HTTP terlampaui",
                    e,
                ))
            })
    }

    fn health_threshold(&self, success: bool) -> usize {
        if success {
            self.success_threshold
        } else {
            self.failure_threshold
        }
    }
}
