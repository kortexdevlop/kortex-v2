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
