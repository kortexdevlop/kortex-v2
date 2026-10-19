#![cfg(not(target_arch = "wasm32"))]
use kortex_platform::native::{self, HttpRequest};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};

#[test]
fn output_creation_is_exclusive_and_confined_and_reads_are_bounded() {
    let directory = std::env::temp_dir().join(format!(
        "kortex-native-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).expect("fixture");
    {
        let first = native::save_output(&directory, "../result.txt", b"first").expect("save");
        let second = native::save_output(&directory, "result.txt", b"second").expect("unique");
        assert_eq!(first.parent(), Some(directory.as_path()));
        assert_ne!(first, second);
        assert_eq!(std::fs::read(first).expect("read"), b"first");
        assert_eq!(
            native::fs::read_limited(&second, 3)
                .expect_err("cap")
                .kind(),
            std::io::ErrorKind::FileTooLarge
        );
        assert_eq!(
            native::fs::read_limited(second, 6).expect("bounded"),
            b"second"
        );
        assert!(native::save_output(&directory, "..", b"unsafe").is_err());
        let mut threads = Vec::new();
        for _ in 0..4 {
            let directory = directory.clone();
            threads.push(std::thread::spawn(move || {
                native::save_output(&directory, "concurrent.txt", b"data").expect("exclusive")
            }));
        }
        let paths = threads
            .into_iter()
            .map(|t| t.join().expect("thread"))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(paths.len(), 4);
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}
#[test]
fn native_http_preserves_status_headers_and_caps_response_without_remote_redirects() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture");
    let address = listener.local_addr().expect("address");
    let server = std::thread::spawn(move || {
        for i in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("timeout");
            let mut raw = Vec::new();
            let mut byte = [0];
            while !raw.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).expect("headers");
                raw.push(byte[0]);
            }
            let headers = String::from_utf8(raw).expect("utf8");
            assert!(headers.to_lowercase().contains("x-kortex-request: 1"));
            let response = match i {
                0 => {
                    "HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                }
                1 => "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\n12345678",
                _ => {
                    "HTTP/1.1 302 Found\r\nLocation: http://remote.invalid/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                }
            };
            stream.write_all(response.as_bytes()).expect("response");
        }
    });
    let url = format!("http://{address}/test");
    let request = |limit| HttpRequest {
        method: "GET",
        url: &url,
        headers: &[("X-Kortex-Request", "1")],
        body: None,
        timeout: Duration::from_secs(5),
        max_bytes: limit,
    };
    assert_eq!(
        native::http(request(1024)).expect("status is data").status,
        401
    );
    assert!(native::http(request(3)).is_err());
    assert_eq!(
        native::http(request(1024))
            .expect("no remote redirect")
            .status,
        302
    );
    server.join().expect("server");
    assert!(native::is_local_http("http://[::1]:8787/a"));
    assert!(!native::is_local_http("http://localhost.evil/a"));
    assert!(!native::is_local_http("http://localhost@evil/a"));
    assert!(
        native::http(HttpRequest {
            url: "http://remote.invalid",
            ..request(10)
        })
        .is_err()
    );
}

#[test]
fn put_and_patch_send_their_body_and_return_status_and_data() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let server = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("timeout");
            let mut raw = Vec::new();
            let mut byte = [0];
            while !raw.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).expect("headers");
                raw.push(byte[0]);
            }
            let head = String::from_utf8(raw).expect("utf8");
            let length: usize = head
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_owned)
                })
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            let mut body = vec![0; length];
            stream.read_exact(&mut body).expect("body");
            seen.push((
                head.lines().next().unwrap_or("").to_owned(),
                String::from_utf8(body).expect("utf8"),
            ));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .expect("response");
        }
        seen
    });
    let url = format!("http://{address}/item");
    for (method, body) in [("PUT", "one"), ("PATCH", "two")] {
        let response = native::http(HttpRequest {
            method,
            url: &url,
            headers: &[("Content-Type", "text/plain")],
            body: Some(body.as_bytes()),
            timeout: Duration::from_secs(5),
            max_bytes: 100,
        })
        .expect("request");
        assert_eq!(
            (response.status, response.body.as_slice()),
            (200, &b"ok"[..])
        );
    }
    let seen = server.join().expect("server");
    assert!(seen[0].0.starts_with("PUT /item") && seen[0].1 == "one");
    assert!(seen[1].0.starts_with("PATCH /item") && seen[1].1 == "two");
}
