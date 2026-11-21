mod support;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicU16, AtomicUsize, Ordering},
    mpsc,
};
use std::thread;
use std::time::Instant;

use support::{TIMEOUT, start_proxy};
use tiny_http::{Method, Response, Server};

struct Origin {
    server: Arc<Server>,
    health: Arc<AtomicU16>,
    traffic: Arc<AtomicUsize>,
    probes: mpsc::Receiver<u16>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Origin {
    fn new(marker: &'static str, initial_status: u16) -> Self {
        let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
        let health = Arc::new(AtomicU16::new(initial_status));
        let traffic = Arc::new(AtomicUsize::new(0));
        let (observed, probes) = mpsc::channel();
        let worker = {
            let server = Arc::clone(&server);
            let health = Arc::clone(&health);
            let traffic = Arc::clone(&traffic);
            thread::spawn(move || {
                let host = server.server_addr().to_string();
                while let Ok(request) = server.recv() {
                    match request.url() {
                        "/health" => {
                            let valid = request.method() == &Method::Get
                                && request
                                    .headers()
                                    .iter()
                                    .any(|h| h.field.equiv("Host") && h.value.as_str() == host);
                            let status = if valid {
                                health.load(Ordering::Relaxed)
                            } else {
                                400
                            };
                            request.respond(Response::empty(status)).unwrap();
                            observed.send(status).unwrap();
                        }
                        "/who" => {
                            traffic.fetch_add(1, Ordering::Relaxed);
                            request.respond(Response::from_string(marker)).unwrap();
                        }
                        _ => request.respond(Response::empty(404)).unwrap(),
                    }
                }
            })
        };
        Self {
            server,
            health,
            traffic,
            probes,
            worker: Some(worker),
        }
    }

    fn addr(&self) -> SocketAddr {
        self.server.server_addr().to_ip().unwrap()
    }

    fn set_health_and_wait(&self, status: u16) {
        self.health.store(status, Ordering::Relaxed);
        let deadline = Instant::now() + TIMEOUT;
        // Ketika probe kedua tiba, Pingora sudah menerapkan hasil probe pertama.
        let mut matching = 0;
        while matching < 2 {
            let seen = self
                .probes
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if seen == status {
                matching += 1;
            }
        }
    }
}

impl Drop for Origin {
    fn drop(&mut self) {
        self.server.unblock();
        self.worker.take().unwrap().join().unwrap();
    }
}

fn fetch(proxy: SocketAddr) -> (u16, String) {
    let mut client = TcpStream::connect(proxy).unwrap();
    client.set_read_timeout(Some(TIMEOUT)).unwrap();
    client
        .write_all(b"GET /who HTTP/1.1\r\nHost: frontend.test\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, body.to_owned())
}

#[test]
fn excludes_http_500_and_rejoins_after_http_200() {
    let a = Origin::new("A", 200);
    let b = Origin::new("B", 200);
    let (_proxy, client) = start_proxy(&[a.addr(), b.addr()], &["--health-check", "http"]);
    let proxy = client.peer_addr().unwrap();
    drop(client);

    for expected in ["A", "B", "A", "B"] {
        assert_eq!(fetch(proxy), (200, expected.into()));
    }

    b.set_health_and_wait(500);
    for _ in 0..10 {
        assert_eq!(fetch(proxy), (200, "A".into()));
    }
    assert_eq!(b.traffic.load(Ordering::Relaxed), 2);

    b.set_health_and_wait(200);
    for expected in ["B", "A", "B", "A", "B", "A"] {
        assert_eq!(fetch(proxy), (200, expected.into()));
    }
    assert_eq!(a.traffic.load(Ordering::Relaxed), 15);
    assert_eq!(b.traffic.load(Ordering::Relaxed), 5);
}

#[test]
fn returns_502_when_neither_health_endpoint_returns_200_at_startup() {
    let a = Origin::new("A", 500);
    let b = Origin::new("B", 204);
    let (_proxy, client) = start_proxy(&[a.addr(), b.addr()], &["--health-check", "http"]);
    assert_eq!(fetch(client.peer_addr().unwrap()).0, 502);
    assert_eq!(a.traffic.load(Ordering::Relaxed), 0);
    assert_eq!(b.traffic.load(Ordering::Relaxed), 0);
}
