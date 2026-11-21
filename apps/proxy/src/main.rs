use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};

use clap::Parser;
use pingora::{
    proxy::http_proxy_service, server::Server, services::background::background_service,
};
use proxy_balancer::{HealthCheckMode, RoundRobinPool};
use proxy_http::ReverseProxy;
use proxy_security::IpAccessPolicy;

#[derive(Parser)]
#[command(
    version,
    about = "Reverse proxy HTTP dengan load balancing round robin"
)]
struct Args {
    /// Alamat IP dan port listener HTTP.
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: SocketAddr,

    /// Alamat IP dan port backend HTTP. Ulangi opsi ini sesuai urutan round robin.
    #[arg(
        long = "upstream",
        value_name = "IP:PORT",
        default_value = "127.0.0.1:8000"
    )]
    upstreams: Vec<SocketAddr>,

    /// Periksa port TCP atau kirim GET /health ke tiap backend (HTTP 200 berarti sehat).
    #[arg(long, value_name = "tcp|http", default_value = "tcp")]
    health_check: HealthCheckMode,

    /// Izinkan IP ini. Jika diisi, hanya IP dalam allowlist yang boleh lewat. Bisa diulang.
    #[arg(long, value_name = "IP")]
    allow_ip: Vec<IpAddr>,

    /// Tolak IP ini dengan HTTP 403, termasuk jika IP juga ada di allowlist. Bisa diulang.
    #[arg(long, value_name = "IP")]
    deny_ip: Vec<IpAddr>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.listen.port() == 0 {
        return Err("Gunakan port listener antara 1 dan 65535.".into());
    }
    let mut seen = HashSet::new();
    for &upstream in &args.upstreams {
        if upstream.port() == 0 || upstream.ip().is_unspecified() {
            return Err(format!(
                "Gunakan IP tujuan yang spesifik dan port 1 sampai 65535 untuk backend {upstream}."
            )
            .into());
        }
        if args.listen == upstream
            || (args.listen.ip().is_unspecified()
                && upstream.ip().is_loopback()
                && (args.listen.is_ipv6() || upstream.is_ipv4())
                && args.listen.port() == upstream.port())
        {
            return Err(format!("Alamat backend {upstream} mengarah kembali ke listener.").into());
        }
        if !seen.insert(upstream) {
            return Err(format!("Alamat backend {upstream} tercantum lebih dari sekali.").into());
        }
    }
    let (&first, remaining) = args
        .upstreams
        .split_first()
        .ok_or("Diperlukan minimal satu backend.")?;
    let policy = IpAccessPolicy::new(args.allow_ip, args.deny_ip);
    let pool = RoundRobinPool::with_health_check(first, remaining, args.health_check);
    let checks = background_service("Health check backend", pool.health_checks());
    let proxy = ReverseProxy::from_pool(pool).with_ip_policy(policy);
    let mut server = Server::new(None)?;
    server.bootstrap();
    let mut service = http_proxy_service(&server.configuration, proxy);
    service.add_tcp(&args.listen.to_string());
    let health = server.add_service(checks);
    server.add_service(service).add_dependency(&health);
    eprintln!(
        "Memulai proxy HTTP di {} menuju {:?} (round robin)",
        args.listen, args.upstreams
    );
    server.run_forever()
}
