use std::{collections::HashMap, str::FromStr, time::Duration};

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
    pub(crate) fn build(self, targets: &[Backend]) -> Box<dyn HealthCheck + Send + Sync> {
        match self {
            Self::Tcp => TcpHealthCheck::new(),
            Self::Http => Box::new(HttpEndpointChecks::new(targets)),
        }
    }
}

// Setiap backend memiliki header Host sendiri. Probe dipakai ulang antarputaran.
struct HttpEndpointChecks {
    probes: HashMap<SocketAddr, HttpHealthCheck>,
}

impl HttpEndpointChecks {
    fn new(targets: &[Backend]) -> Self {
        let probes = targets
            .iter()
            .map(|target| {
                let mut probe = HttpHealthCheck::new(&target.addr.to_string(), false);
                probe
                    .req
                    .set_raw_path(b"/health")
                    .expect("path /health valid");
                (target.addr.clone(), probe)
            })
            .collect();
        Self { probes }
    }
}

#[async_trait]
impl HealthCheck for HttpEndpointChecks {
    async fn check(&self, target: &Backend) -> Result<()> {
        let probe = self.probes.get(&target.addr).ok_or_else(|| {
            Error::explain(ErrorType::InternalError, "Backend probe belum terdaftar")
        })?;
        tokio::time::timeout(Duration::from_secs(2), probe.check(target))
            .await
            .map_err(|e| {
                Error::because(
                    ErrorType::ReadTimedout,
                    "Batas waktu probe /health terlampaui",
                    e,
                )
            })?
    }

    fn health_threshold(&self, _success: bool) -> usize {
        1
    }
}
