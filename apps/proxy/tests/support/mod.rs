use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

pub(super) const TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct Proxy(Child);

impl Drop for Proxy {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(super) fn start_proxy(upstreams: &[SocketAddr], options: &[&str]) -> (Proxy, TcpStream) {
    let listen = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_proxy"));
    command
        .arg("--listen")
        .arg(listen.to_string())
        .args(options);
    for address in upstreams {
        command.arg("--upstream").arg(address.to_string());
    }
    let mut proxy = Proxy(command.spawn().unwrap());
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(status) = proxy.0.try_wait().unwrap() {
            panic!("Proxy berhenti saat startup: {status}");
        }
        if let Ok(stream) = TcpStream::connect(listen) {
            stream.set_read_timeout(Some(TIMEOUT)).unwrap();
            stream.set_write_timeout(Some(TIMEOUT)).unwrap();
            return (proxy, stream);
        }
        assert!(
            Instant::now() < deadline,
            "Listener gagal dimulai di {listen}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
