use std::{collections::HashMap, num::NonZeroU32, str::FromStr, time::Duration};

use async_trait::async_trait;
use pingora::lb::{
    Backend,
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
        targets: &[Backend],
        options: &HttpHealthOptions,
    ) -> Result<Box<dyn HealthCheck + Send + Sync>> {
        match self {
            Self::Tcp if *options != HttpHealthOptions::default() => Error::e_explain(
                ErrorType::Custom("Konfigurasi health check"),
                "Opsi path, interval, dan timeout HTTP memerlukan --health-check http",
            ),
            Self::Tcp => Ok(TcpHealthCheck::new()),
            Self::Http => Ok(Box::new(HttpEndpointChecks::new(targets, options)?)),
        }
    }
}

// Setiap backend memiliki header Host sendiri. Probe dipakai ulang antarputaran.
struct HttpEndpointChecks {
    probes: HashMap<SocketAddr, HttpHealthCheck>,
    timeout: Duration,
}

impl HttpEndpointChecks {
    fn new(targets: &[Backend], options: &HttpHealthOptions) -> Result<Self> {
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
        let mut probes = HashMap::with_capacity(targets.len());
        for target in targets {
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
            .map_err(|e| {
                Error::because(
                    ErrorType::ReadTimedout,
                    "Batas waktu probe HTTP terlampaui",
                    e,
                )
            })?
    }

    fn health_threshold(&self, _success: bool) -> usize {
        1
    }
}
