//! A tiny local webhook endpoint for desktop apps: `POST http://127.0.0.1:<port>/hook/<token>`.
//! Only the loopback address is bound, the token is a secret path, requests that carry an `Origin`
//! header (any web page) or a foreign `Host` (DNS rebinding) are refused, and the body is capped.
//! It hands the body to a callback; what the app does with it is the app's business.
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Largest body accepted.
pub const MAX_BODY: usize = 65_536;
const MAX_HEAD: usize = 16_384;

/// A running endpoint; dropping it stops the server.
pub struct Hook {
    pub port: u16,
    stop: Arc<AtomicBool>,
}

impl Drop for Hook {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the blocking `accept` so the thread can see the flag and end.
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)),
            Duration::from_millis(500),
        );
    }
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
}

/// Reads one request and answers it. Returns the body when it was an accepted call.
fn handle(stream: &mut TcpStream, token: &str) -> Option<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 2048];
    let head_end = loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        raw.extend_from_slice(&chunk[..read]);
        if let Some(at) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if raw.len() > MAX_HEAD {
            respond(
                stream,
                "431 Request Header Fields Too Large",
                "{\"error\":\"head_too_large\"}",
            );
            return None;
        }
    };
    let head = String::from_utf8_lossy(&raw[..head_end]).into_owned();
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let (method, path) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    );
    let header = |name: &str| {
        lines
            .clone()
            .find_map(|l| {
                l.split_once(':')
                    .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            })
            .map(|(_, v)| v.trim().to_owned())
    };
    // A web page can send a request to 127.0.0.1 but cannot omit its Origin: refuse those, and a
    // Host that is not the loopback address (DNS rebinding).
    let host_ok = header("host").is_some_and(|h| {
        let name = h.rsplit_once(':').map_or(h.as_str(), |(n, _)| n);
        name == "127.0.0.1" || name == "localhost"
    });
    if header("origin").is_some() || !host_ok {
        respond(stream, "403 Forbidden", "{\"error\":\"forbidden\"}");
        return None;
    }
    let expected = format!("/hook/{token}");
    // Same length and same bytes, compared without stopping at the first difference.
    let same = path.len() == expected.len()
        && path
            .bytes()
            .zip(expected.bytes())
            .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
            == 0;
    if method != "POST" || !same {
        respond(stream, "404 Not Found", "{\"error\":\"not_found\"}");
        return None;
    }
    let length: usize = header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        respond(
            stream,
            "413 Payload Too Large",
            "{\"error\":\"payload_too_large\"}",
        );
        return None;
    }
    let mut body = raw[head_end..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);
    respond(stream, "202 Accepted", "{\"queued\":true}");
    Some(String::from_utf8_lossy(&body).into_owned())
}

/// Starts the endpoint on `127.0.0.1:<port>` (`0` picks a free port, see [`Hook::port`]). Every
/// accepted call hands its body to `on_call`.
///
/// # Errors
/// The port cannot be bound.
pub fn listen(
    port: u16,
    token: String,
    on_call: impl Fn(String) + Send + 'static,
) -> std::io::Result<Hook> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
    let port = listener.local_addr()?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    std::thread::Builder::new()
        .name("kortex-webhook".into())
        .spawn(move || {
            for connection in listener.incoming() {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = connection else { continue };
                if let Some(body) = handle(&mut stream, &token) {
                    on_call(body);
                }
            }
        })?;
    Ok(Hook { port, stop })
}
