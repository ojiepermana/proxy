mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};
use support::{TIMEOUT, start_proxy};

use tiny_http::{Header, Method, Response, Server};

#[test]
fn forwards_post_and_returns_backend_response() {
    let backend = Server::http("127.0.0.1:0").unwrap();
    let (_proxy, mut client) = start_proxy(
        &[backend.server_addr().to_ip().unwrap()],
        &[
            "--allow-ip",
            "::1",
            "--allow-ip",
            "127.0.0.1",
            "--deny-ip",
            "::1",
        ],
    );
    let responder = thread::spawn(move || {
        let mut request = backend.recv_timeout(TIMEOUT).unwrap().expect("request");
        assert_eq!(request.method(), &Method::Post);
        assert_eq!(request.url(), "/orders/42?view=full");
        let request_id = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("X-Request-Id"))
            .unwrap();
        assert_eq!(request_id.value.as_str(), "forward-42");
        let mut body = String::new();
        request.as_reader().read_to_string(&mut body).unwrap();
        assert_eq!(body, "quantity=2");
        let response = Response::from_string("order accepted")
            .with_status_code(201)
            .with_header(Header::from_bytes("X-Backend", "local").unwrap());
        request.respond(response).unwrap();
    });

    client
        .write_all(
            concat!(
                "POST /orders/42?view=full HTTP/1.1\r\n",
                "Host: frontend.test\r\nX-Request-Id: forward-42\r\n",
                "X-Forwarded-For: ::1\r\n",
                "Connection: close\r\nContent-Length: 10\r\n\r\nquantity=2"
            )
            .as_bytes(),
        )
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    responder.join().unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with("HTTP/1.1 201 "), "{headers}");
    assert!(
        headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case("X-Backend: local"))
    );
    assert_eq!(body, "order accepted");
}

#[test]
fn returns_bad_gateway_when_backend_is_offline() {
    let backend = TcpListener::bind("127.0.0.1:0").unwrap();
    let (_proxy, mut client) = start_proxy(&[backend.local_addr().unwrap()], &[]);
    drop(backend);

    client
        .write_all(b"GET / HTTP/1.1\r\nHost: frontend.test\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 502 "), "{response}");
}

#[test]
fn alternates_backends_per_request_on_one_connection() {
    let backend_a = Server::http("127.0.0.1:0").unwrap();
    let backend_b = Server::http("127.0.0.1:0").unwrap();
    let backends = [(&backend_a, "A"), (&backend_b, "B")];
    let addresses = backends.map(|(server, _)| server.server_addr().to_ip().unwrap());
    let (_proxy, client) = start_proxy(&addresses, &[]);
    let mut client = BufReader::new(client);

    for (turn, (expected_server, marker)) in backends.iter().cycle().take(10).enumerate() {
        let path = format!("/turn/{turn}");
        write!(
            client.get_mut(),
            "GET {path} HTTP/1.1\r\nHost: frontend.test\r\n\r\n"
        )
        .unwrap();
        let request = expected_server
            .recv_timeout(TIMEOUT)
            .unwrap()
            .unwrap_or_else(|| panic!("Request {turn} belum diterima backend {marker}"));
        assert_eq!(request.url(), path);
        request.respond(Response::from_string(*marker)).unwrap();

        assert_eq!(read_marker(&mut client), marker.as_bytes()[0]);
    }
}

#[test]
fn routes_to_survivor_and_resumes_round_robin_after_recovery() {
    let a = Server::http("127.0.0.1:0").unwrap();
    let b = Server::http("127.0.0.1:0").unwrap();
    let b_addr = b.server_addr().to_ip().unwrap();
    let (_proxy, client) = start_proxy(&[a.server_addr().to_ip().unwrap(), b_addr], &[]);
    let mut client = BufReader::new(client);
    let backends = [(&a, b'A'), (&b, b'B')];
    for expected in b"ABABA" {
        assert_eq!(fetch_marker(&mut client, &backends), *expected);
    }

    drop(b);
    let deadline = Instant::now() + TIMEOUT;
    while TcpStream::connect(b_addr).is_ok() {
        assert!(Instant::now() < deadline, "Backend B masih hidup");
        thread::sleep(Duration::from_millis(10));
    }
    for _ in 0..10 {
        assert_eq!(fetch_marker(&mut client, &[(&a, b'A')]), b'A');
        // Rentangkan trafik hingga melewati satu putaran probe selama B mati.
        thread::sleep(Duration::from_millis(150));
    }

    // Tanpa request klien, backend yang hidup kembali harus menerima probe TCP.
    let listener = TcpListener::bind(b_addr).unwrap();
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + TIMEOUT;
    let mut probe = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "Probe backend B tidak tiba");
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("{e}"),
        }
    };
    probe.set_read_timeout(Some(TIMEOUT)).unwrap();
    assert_eq!(probe.read(&mut [0]).unwrap(), 0);
    listener.set_nonblocking(false).unwrap();
    let b = Server::from_listener(listener, None).unwrap();
    let backends = [(&a, b'A'), (&b, b'B')];
    let deadline = Instant::now() + TIMEOUT;
    while fetch_marker(&mut client, &backends) != b'B' {
        assert!(
            Instant::now() < deadline,
            "Backend B belum kembali ke rotasi"
        );
        thread::sleep(Duration::from_millis(20));
    }
    for expected in b"ABABABAB" {
        assert_eq!(fetch_marker(&mut client, &backends), *expected);
    }
}

fn fetch_marker(client: &mut BufReader<TcpStream>, backends: &[(&Server, u8)]) -> u8 {
    client
        .get_mut()
        .write_all(b"GET /who HTTP/1.1\r\nHost: frontend.test\r\n\r\n")
        .unwrap();
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some((request, marker)) = backends.iter().find_map(|(server, marker)| {
            server.try_recv().unwrap().map(|request| (request, *marker))
        }) {
            assert_eq!(request.url(), "/who");
            // Tutup koneksi upstream setiap kali agar penghentian backend uji juga menutup koneksi lamanya.
            let mut writer = request.into_writer();
            writer
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n")
                .unwrap();
            writer.write_all(&[marker]).unwrap();
            writer.flush().unwrap();
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Request tidak tiba di backend sehat"
        );
        thread::sleep(Duration::from_millis(2));
    }
    read_marker(client)
}

fn read_marker(client: &mut BufReader<TcpStream>) -> u8 {
    let mut headers = (&mut *client).lines().map(Result::unwrap);
    assert!(headers.next().unwrap().starts_with("HTTP/1.1 200 "));
    assert!(headers.any(|line| line.is_empty()));
    let mut body = [0];
    client.read_exact(&mut body).unwrap();
    body[0]
}

#[test]
fn blocks_denied_and_unlisted_peers_before_forwarding() {
    let backend = Server::http("127.0.0.1:0").unwrap();
    let addresses = [backend.server_addr().to_ip().unwrap()];
    for rules in [
        &["--deny-ip", "127.0.0.1"][..],
        &["--allow-ip", "::1"],
        &[
            "--allow-ip",
            "127.0.0.1",
            "--deny-ip",
            "::1",
            "--deny-ip",
            "127.0.0.1",
        ],
    ] {
        let (_proxy, mut client) = start_proxy(&addresses, rules);
        client
            .write_all(b"GET /blocked HTTP/1.1\r\nHost: frontend.test\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(
            response.starts_with("HTTP/1.1 403 "),
            "{rules:?}: {response}"
        );
        assert!(backend.try_recv().unwrap().is_none(), "{rules:?}");
    }
}
