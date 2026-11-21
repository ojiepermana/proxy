mod support;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

use support::{TIMEOUT, start_proxy};
use tiny_http::{Method, Request, Response, Server};

enum HealthReply {
    Status(u16),
    Hold(mpsc::Sender<Request>),
}

struct Origin {
    server: Arc<Server>,
    health: Arc<Mutex<HealthReply>>,
    traffic: Arc<AtomicUsize>,
    probes: mpsc::Receiver<u16>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Origin {
    fn new(marker: &'static str, health_path: &'static str, initial_status: u16) -> Self {
        let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
        let health = Arc::new(Mutex::new(HealthReply::Status(initial_status)));
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
                        path if path == health_path => {
                            let valid = request.method() == &Method::Get
                                && request
                                    .headers()
                                    .iter()
                                    .any(|h| h.field.equiv("Host") && h.value.as_str() == host);
                            if !valid {
                                request.respond(Response::empty(400)).unwrap();
                                continue;
                            }
                            match &*health.lock().unwrap() {
                                HealthReply::Status(status) => {
                                    request.respond(Response::empty(*status)).unwrap();
                                    observed.send(*status).unwrap();
                                }
                                HealthReply::Hold(held) => {
                                    let _ = held.send(request);
                                }
                            }
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
        {
            let mut reply = self.health.lock().unwrap();
            while self.probes.try_recv().is_ok() {}
            *reply = HealthReply::Status(status);
        }
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

    fn hold_health(&self) -> mpsc::Receiver<Request> {
        let (held, pending) = mpsc::channel();
        *self.health.lock().unwrap() = HealthReply::Hold(held);
        pending
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
    let a = Origin::new("A", "/health", 200);
    let b = Origin::new("B", "/health", 200);
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
    let a = Origin::new("A", "/health", 500);
    let b = Origin::new("B", "/health", 204);
    let (_proxy, client) = start_proxy(&[a.addr(), b.addr()], &["--health-check", "http"]);
    assert_eq!(fetch(client.peer_addr().unwrap()).0, 502);
    assert_eq!(a.traffic.load(Ordering::Relaxed), 0);
    assert_eq!(b.traffic.load(Ordering::Relaxed), 0);
}

#[test]
fn excludes_timed_out_ready_endpoint_and_rejoins_after_recovery() {
    let a = Origin::new("A", "/ready", 200);
    let b = Origin::new("B", "/ready", 200);
    let options = [
        "--health-check",
        "http",
        "--health-check-path",
        "/ready",
        "--health-check-interval-ms",
        "100",
        "--health-check-timeout-ms",
        "150",
    ];
    let (_proxy, client) = start_proxy(&[a.addr(), b.addr()], &options);
    let proxy = client.peer_addr().unwrap();
    for expected in ["A", "B", "A", "B"] {
        assert_eq!(fetch(proxy), (200, expected.into()));
    }

    let pending = b.hold_health();
    let first = pending.recv_timeout(TIMEOUT).unwrap();
    // Probe berikutnya hanya dapat tiba setelah timeout dan jeda antarputaran.
    let _next = pending.recv_timeout(Duration::from_millis(900)).unwrap();
    for _ in 0..12 {
        assert_eq!(fetch(proxy), (200, "A".into()));
    }
    assert_eq!(b.traffic.load(Ordering::Relaxed), 2);
    let _ = first.respond(Response::empty(200));
    b.set_health_and_wait(200);
    for expected in ["B", "A", "B", "A", "B", "A"] {
        assert_eq!(fetch(proxy), (200, expected.into()));
    }
}

#[test]
fn applies_consecutive_thresholds_to_ready_probes() {
    let a = Origin::new("A", "/ready", 200);
    let b = Origin::new("B", "/ready", 200);
    let options = [
        "--health-check",
        "http",
        "--health-check-path",
        "/ready",
        "--health-check-interval-ms",
        "25",
        "--health-check-timeout-ms",
        "10000",
        "--health-check-failure-threshold",
        "3",
        "--health-check-success-threshold",
        "2",
    ];
    let (_proxy, client) = start_proxy(&[a.addr(), b.addr()], &options);
    let proxy = client.peer_addr().unwrap();
    let pending = b.hold_health();
    let mut probe = pending.recv_timeout(TIMEOUT).unwrap();

    for (step, (status, expected)) in [
        (500, "ABAB"),
        (500, "ABAB"),
        (200, "ABAB"), // Memutus rentetan gagal.
        (500, "ABAB"),
        (500, "ABAB"),
        (500, "AAAA"),
        (200, "AAAA"),
        (500, "AAAA"), // Memutus rentetan sukses.
        (200, "AAAA"),
        (200, "BABA"),
    ]
    .into_iter()
    .enumerate()
    {
        probe.respond(Response::empty(status)).unwrap();
        // Tahan probe berikutnya agar trafik mencerminkan tepat satu hasil baru.
        probe = pending.recv_timeout(TIMEOUT).unwrap();
        for marker in expected.chars() {
            assert_eq!(
                fetch(proxy),
                (200, marker.to_string()),
                "hasil probe B ke-{} ({status})",
                step + 1
            );
        }
    }
}

#[test]
fn accepts_ready_response_within_extended_timeout() {
    let a = Origin::new("A", "/ready", 200);
    let pending = a.hold_health();
    let delayed = thread::spawn(move || {
        let request = pending.recv_timeout(TIMEOUT).unwrap();
        // Lebih lama dari batas baca bawaan Pingora yang sebesar satu detik.
        thread::sleep(Duration::from_millis(1200));
        let _ = request.respond(Response::empty(200));
    });
    let options = [
        "--health-check",
        "http",
        "--health-check-path",
        "/ready",
        "--health-check-timeout-ms",
        "3000",
    ];
    let (_proxy, client) = start_proxy(&[a.addr()], &options);
    assert_eq!(fetch(client.peer_addr().unwrap()), (200, "A".into()));
    delayed.join().unwrap();
}
