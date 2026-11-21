//! Penerusan request HTTP ke backend yang dipilih oleh [`proxy_balancer`].

use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use pingora::{
    Error, ErrorType, Result,
    http::{Method, ResponseHeader},
    prelude::HttpPeer,
    proxy::{ProxyHttp, Session},
};
use proxy_balancer::RoundRobinPool;
use proxy_security::IpAccessPolicy;
use serde_json::json;

/// Handler Pingora yang berbagi satu pool untuk semua request.
pub struct ReverseProxy {
    backends: RoundRobinPool,
    ip_policy: IpAccessPolicy,
}

/// Nama handler yang dipakai oleh konfigurasi satu backend.
pub type SingleBackend = ReverseProxy;

/// Backend terpilih dan kegagalan koneksi sebelum request dikirim.
#[derive(Default)]
pub struct RequestBackend {
    selected: Option<SocketAddr>,
    failed: Vec<SocketAddr>,
}

impl ReverseProxy {
    pub fn new(address: SocketAddr) -> Self {
        Self::with_backends(address, &[])
    }

    pub fn with_backends(first: SocketAddr, remaining: &[SocketAddr]) -> Self {
        Self::from_pool(RoundRobinPool::new(first, remaining))
    }

    pub fn from_pool(backends: RoundRobinPool) -> Self {
        Self {
            backends,
            ip_policy: IpAccessPolicy::default(),
        }
    }

    pub fn with_ip_policy(mut self, policy: IpAccessPolicy) -> Self {
        self.ip_policy = policy;
        self
    }

    async fn respond_status(&self, session: &mut Session) -> Result<()> {
        let backends: Vec<_> = self
            .backends
            .backend_probe_statuses()
            .map(|status| {
                json!({
                    "address": status.address,
                    "healthy": status.healthy,
                    "consecutive_successes": status.consecutive_successes,
                    "consecutive_failures": status.consecutive_failures,
                })
            })
            .collect();
        let body = json!({ "backends": backends }).to_string();
        let mut header = ResponseHeader::build(200, Some(3))?;
        header.insert_header("Content-Type", "application/json")?;
        header.insert_header("Cache-Control", "no-store")?;
        header.insert_header("Content-Length", body.len().to_string())?;
        let head_only = session.req_header().method == Method::HEAD;
        session
            .write_response_header(Box::new(header), head_only)
            .await?;
        if !head_only {
            session.write_response_body(Some(body.into()), true).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl ProxyHttp for ReverseProxy {
    type CTX = RequestBackend;

    fn new_ctx(&self) -> Self::CTX {
        RequestBackend::default()
    }

    async fn request_filter(&self, session: &mut Session, _ctx: &mut Self::CTX) -> Result<bool> {
        let peer_ip = session
            .client_addr()
            .and_then(|addr| addr.as_inet())
            .map(SocketAddr::ip);
        if !self.ip_policy.allows(peer_ip) {
            session.respond_error(403).await?;
            return Ok(true);
        }

        let request = session.req_header();
        if request.uri.path() == "/status" && matches!(request.method, Method::GET | Method::HEAD) {
            self.respond_status(session).await?;
            return Ok(true);
        }
        Ok(false)
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let address = ctx
            .selected
            .or_else(|| self.backends.select())
            .ok_or_else(|| Error::explain(ErrorType::HTTPStatus(502), "Tidak ada backend sehat"))?;
        ctx.selected = Some(address);
        let mut peer = HttpPeer::new(address, false, String::new());
        peer.options.connection_timeout = Some(Duration::from_secs(1));
        Ok(Box::new(peer))
    }

    fn fail_to_connect(
        &self,
        _session: &mut Session,
        _peer: &HttpPeer,
        ctx: &mut Self::CTX,
        mut error: Box<Error>,
    ) -> Box<Error> {
        if let Some(address) = ctx.selected.take() {
            ctx.failed.push(address);
            ctx.selected = self.backends.select_excluding(&ctx.failed);
        }
        error.set_retry(ctx.selected.is_some());
        error
    }
}
