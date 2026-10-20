use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::Duration;

use kortex_platform::webhook::listen;

fn request(port: u16, raw: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    stream.write_all(raw.as_bytes()).expect("write");
    let mut out = String::new();
    let _ = stream.read_to_string(&mut out);
    out
}

#[test]
fn only_a_local_non_browser_call_with_the_secret_path_gets_through() {
    let (tx, rx) = mpsc::channel::<String>();
    let hook = listen(0, "s3cr3t".into(), move |body| {
        let _ = tx.send(body);
    })
    .expect("listen");
    let port = hook.port;
    let post = |path: &str, extra: &str, body: &str| {
        format!(
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\n{extra}\r\n{body}",
            body.len()
        )
    };
    // The right path, no Origin: accepted and delivered.
    let ok = request(port, &post("/hook/s3cr3t", "", "{\"n\":1}"));
    assert!(ok.starts_with("HTTP/1.1 202"), "{ok}");
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).expect("delivered"),
        "{\"n\":1}"
    );
    // A web page (it always sends an Origin), a wrong token, a wrong method, a foreign host: refused.
    assert!(
        request(
            port,
            &post("/hook/s3cr3t", "Origin: https://evil.example\r\n", "x")
        )
        .starts_with("HTTP/1.1 403")
    );
    assert!(request(port, &post("/hook/wrong", "", "x")).starts_with("HTTP/1.1 404"));
    assert!(request(port, &post("/hook/s3cr3t/more", "", "x")).starts_with("HTTP/1.1 404"));
    assert!(
        request(
            port,
            &format!("GET /hook/s3cr3t HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n")
        )
        .starts_with("HTTP/1.1 404")
    );
    let rebound = format!(
        "POST /hook/s3cr3t HTTP/1.1\r\nHost: attacker.example:{port}\r\nContent-Length: 1\r\n\r\nx"
    );
    assert!(request(port, &rebound).starts_with("HTTP/1.1 403"));
    // Too big a body is refused before it is read.
    let big = format!(
        "POST /hook/s3cr3t HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Length: 999999\r\n\r\n"
    );
    assert!(request(port, &big).starts_with("HTTP/1.1 413"));
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "nothing else was delivered"
    );
    // Dropping the hook closes the port.
    drop(hook);
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
            Duration::from_millis(300)
        )
        .is_err()
    );
}
