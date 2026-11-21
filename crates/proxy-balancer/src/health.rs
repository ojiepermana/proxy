use std::{
    collections::HashMap,
    num::{NonZeroU32, NonZeroUsize},
    str::FromStr,
    sync::{Arc, Mutex, Weak},
    time::Duration,
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

// Setiap backend memiliki header Host sendiri. Probe dipakai ulang antarputaran.
struct HttpEndpointChecks {
    probes: HashMap<SocketAddr, HttpHealthCheck>,
    timeout: Duration,
    failure_threshold: usize,
    success_threshold: usize,
    startup: Option<StartupAdmission>,
}

// Hanya backend yang belum pernah masuk rotasi yang memiliki penghitung di sini.
struct StartupAdmission {
    view: Weak<Backends>,
    successes: Mutex<HashMap<SocketAddr, usize>>,
}

impl StartupAdmission {
    fn new(view: &Arc<Backends>) -> Self {
        let mut successes = HashMap::new();
        for target in view.get_backend().iter() {
            view.set_enable(target, false);
            successes.insert(target.addr.clone(), 0);
        }
        Self {
            view: Arc::downgrade(view),
            successes: Mutex::new(successes),
        }
    }

    fn observe(&self, target: &Backend, success: bool, threshold: usize) {
        let mut pending = self.successes.lock().expect("penghitung probe startup");
        let Some(streak) = pending.get_mut(&target.addr) else {
            return;
        };
        if !success {
            *streak = 0;
            return;
        }
        *streak += 1;
        if *streak == threshold {
            pending.remove(&target.addr);
            if let Some(view) = self.view.upgrade() {
                view.set_enable(target, true);
            }
        }
    }
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
            startup: options.start_unhealthy.then(|| StartupAdmission::new(view)),
        })
    }
}

#[async_trait]
impl HealthCheck for HttpEndpointChecks {
    async fn check(&self, target: &Backend) -> Result<()> {
        let probe = self.probes.get(&target.addr).ok_or_else(|| {
            Error::explain(ErrorType::InternalError, "Backend probe belum terdaftar")
        })?;
        let result = tokio::time::timeout(self.timeout, probe.check(target))
            .await
            .unwrap_or_else(|e| {
                Err(Error::because(
                    ErrorType::ReadTimedout,
                    "Batas waktu probe HTTP terlampaui",
                    e,
                ))
            });
        if let Some(startup) = &self.startup {
            startup.observe(target, result.is_ok(), self.success_threshold);
        }
        result
    }

    fn health_threshold(&self, success: bool) -> usize {
        if success {
            self.success_threshold
        } else {
            self.failure_threshold
        }
    }
}
