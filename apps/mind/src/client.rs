//! Transports to the AI gateway: a streamed `fetch` in the browser (the session cookie never
//! reaches Rust), a streamed native HTTP call with the device token on desktop. Both feed the
//! same [`collect`], which turns the gateway's events into one assistant turn.
use std::future::Future;

use kortex_ai::chat::{ApiError, Request};
use kortex_ai::credits::Credits;
use kortex_ai::stream::{Assembler, Event, SseParser, Turn};

/// How a request authenticates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Auth {
    /// Browser: the session cookie travels by itself; `user` pins the expected account.
    Web { api: String, user: Option<String> },
    /// Desktop: the device token stored by `kortex login`.
    Device { api: String, token: String },
}

impl Auth {
    #[must_use]
    pub fn api(&self) -> &str {
        match self {
            Self::Web { api, .. } | Self::Device { api, .. } => api,
        }
    }
}

/// One step of a response, whatever the transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Status(u16),
    Text(String),
    End,
    Failed,
}

pub trait Source {
    fn next(&mut self) -> impl Future<Output = Piece>;
}

/// The gateway or the network refused: `status` is 0 for a network failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskError {
    pub status: u16,
    pub code: String,
}

fn network(code: &str) -> AskError {
    AskError {
        status: 0,
        code: code.to_owned(),
    }
}

/// Reads one response to the end. `on_event` sees every event as it arrives (live text).
///
/// # Errors
/// [`AskError`]: the gateway's error code (`ai_paused`, `user_empty`, `consent_required`…), an
/// `error` event in the stream, or a network failure / truncated stream.
pub async fn collect(
    source: &mut impl Source,
    mut on_event: impl FnMut(&Event),
) -> Result<(Turn, Option<Credits>), AskError> {
    let status = loop {
        match source.next().await {
            Piece::Status(status) => break status,
            Piece::Text(_) => {}
            Piece::End | Piece::Failed => return Err(network("network")),
        }
    };
    if status != 200 {
        let mut body = String::new();
        loop {
            match source.next().await {
                Piece::Text(text) => body.push_str(&text),
                Piece::Status(_) => {}
                Piece::End | Piece::Failed => break,
            }
        }
        let ApiError { status, code } = ApiError::parse(status, &body);
        return Err(AskError { status, code });
    }
    let mut parser = SseParser::default();
    let mut assembler = Assembler::default();
    let mut credits = None;
    let mut complete = false;
    loop {
        match source.next().await {
            Piece::Text(text) => {
                for event in parser.feed(&text) {
                    match &event {
                        Event::Error(code) => {
                            return Err(AskError {
                                status: 200,
                                code: code.clone(),
                            });
                        }
                        Event::Done(c) => {
                            credits = *c;
                            complete = true;
                        }
                        Event::Delta { .. } | Event::Finish(_) => {}
                    }
                    assembler.push(&event);
                    on_event(&event);
                }
            }
            Piece::End => break,
            Piece::Failed => return Err(network("network")),
            Piece::Status(_) => {}
        }
    }
    if complete {
        Ok((assembler.finish(), credits))
    } else {
        Err(network("truncated"))
    }
}

/// Incremental UTF-8 decoding for byte streams: a multi-byte character split across two network
/// chunks is held back until it is complete instead of being replaced by U+FFFD.
#[derive(Debug, Default)]
pub struct Utf8Stream {
    pending: Vec<u8>,
}

impl Utf8Stream {
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    out.push_str(text);
                    self.pending.clear();
                    return out;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    out.push_str(&String::from_utf8_lossy(&self.pending[..valid]));
                    match error.error_len() {
                        // A genuinely invalid byte is dropped; decoding goes on after it.
                        Some(invalid) => {
                            self.pending.drain(..valid + invalid);
                        }
                        // An incomplete character at the end waits for the next chunk.
                        None => {
                            self.pending.drain(..valid);
                            return out;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use std::cell::Cell;

    use kortex_ai::chat::Request;
    use kortex_ai::credits::Credits;
    use kortex_platform::document::{FetchCanceller, FetchEvent, FetchStream, fetch_stream};

    use super::{Account, Auth, Piece, Source};

    thread_local! {
        static ACTIVE: Cell<Option<FetchCanceller>> = const { Cell::new(None) };
    }

    pub struct Stream(FetchStream);

    impl Source for Stream {
        async fn next(&mut self) -> Piece {
            match self.0.next().await {
                FetchEvent::Status(s) => Piece::Status(s),
                FetchEvent::Chunk(text) => Piece::Text(text),
                FetchEvent::End => Piece::End,
                FetchEvent::Failed => Piece::Failed,
            }
        }
    }

    pub fn start(auth: &Auth, request: &Request) -> Result<Stream, String> {
        let Auth::Web { api, user } = auth else {
            return Err("device authentication is not available in the browser".into());
        };
        let stream = fetch_stream(
            &format!("{api}/v1/ai/chat"),
            &request.body(),
            user.as_deref(),
        )
        .map_err(|e| e.0)?;
        ACTIVE.with(|c| c.set(Some(stream.canceller())));
        Ok(Stream(stream))
    }

    pub fn cancel() {
        ACTIVE.with(|c| {
            if let Some(canceller) = c.take() {
                canceller.cancel();
            }
        });
    }

    /// The browser session; a development API base can be given in the page URL (localhost only).
    pub async fn account() -> Account {
        let api = kortex_platform::document::dev_api_override()
            .await
            .unwrap_or_else(|| kortex_auth::api().to_owned());
        match kortex_auth::session_at(&api).await {
            Ok(Some(profile)) => Account::Ready {
                auth: Auth::Web {
                    api,
                    user: Some(profile.id),
                },
                email: profile.email,
            },
            Ok(None) | Err(kortex_auth::Error::SignedOut) => Account::SignedOut,
            Err(_) => Account::Unavailable,
        }
    }

    pub async fn credits(auth: &Auth) -> Option<Credits> {
        let Auth::Web { api, user } = auth else {
            return None;
        };
        let reply = kortex_auth::request_at(api, user.as_deref(), "GET", "/v1/ai/credits", None)
            .await
            .ok()?;
        (reply.status == 200)
            .then(|| serde_json::from_str::<serde_json::Value>(&reply.body).ok())
            .flatten()
            .and_then(|v| Credits::from_json(&v))
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use std::sync::Mutex;
    use std::time::Duration;

    use kortex_ai::chat::Request;
    use kortex_ai::credits::Credits;
    use kortex_platform::native::task::{Channel, Slot, spawn};
    use kortex_platform::native::{HttpRequest, http_stream};
    use kortex_runtime::CancelToken;

    use super::{Account, Auth, Piece, Source, Utf8Stream};

    static ACTIVE: Mutex<Option<CancelToken>> = Mutex::new(None);

    pub struct Stream(Channel<Piece>);

    impl Source for Stream {
        async fn next(&mut self) -> Piece {
            self.0.recv().await.unwrap_or(Piece::Failed)
        }
    }

    pub fn start(auth: &Auth, request: &Request) -> Result<Stream, String> {
        let Auth::Device { api, token } = auth else {
            return Err("cookie authentication is not available on desktop".into());
        };
        let url = format!("{api}/v1/ai/chat");
        let body = request.body();
        let bearer = format!("Bearer {token}");
        let channel = Channel::new();
        let cancel = CancelToken::default();
        if let Ok(mut active) = ACTIVE.lock() {
            *active = Some(cancel.clone());
        }
        let remote = channel.clone();
        spawn("kortex-mind-ai", move || {
            let headers = [
                ("X-Kortex-Request", "1"),
                ("Content-Type", "application/json"),
                ("Authorization", bearer.as_str()),
            ];
            let mut decoder = Utf8Stream::default();
            let result = http_stream(
                HttpRequest {
                    method: "POST",
                    url: &url,
                    headers: &headers,
                    body: Some(body.as_bytes()),
                    timeout: Duration::from_secs(180),
                    max_bytes: 4_000_000,
                },
                |status| remote.send(Piece::Status(status)),
                |bytes| {
                    let text = decoder.push(bytes);
                    if !text.is_empty() {
                        remote.send(Piece::Text(text));
                    }
                    !cancel.is_cancelled()
                },
            );
            remote.send(if result.is_ok() && !cancel.is_cancelled() {
                Piece::End
            } else {
                Piece::Failed
            });
            remote.close();
        })
        .map_err(|e| e.to_string())?;
        Ok(Stream(channel))
    }

    pub fn cancel() {
        if let Ok(mut active) = ACTIVE.lock()
            && let Some(token) = active.take()
        {
            token.cancel();
        }
    }

    /// The device credentials stored by `kortex login` (system keychain).
    pub async fn account() -> Account {
        match kortex_pkg::Home::from_env().and_then(|home| home.load_credentials()) {
            Ok(Some(c)) => Account::Ready {
                auth: Auth::Device {
                    api: c.api,
                    token: c.token,
                },
                email: c.email,
            },
            Ok(None) => Account::SignedOut,
            Err(_) => Account::Unavailable,
        }
    }

    pub async fn credits(auth: &Auth) -> Option<Credits> {
        let Auth::Device { api, token } = auth else {
            return None;
        };
        let (url, bearer) = (format!("{api}/v1/ai/credits"), format!("Bearer {token}"));
        let slot = Slot::new();
        let remote = slot.clone();
        spawn("kortex-mind-credits", move || {
            let headers = [
                ("X-Kortex-Request", "1"),
                ("Authorization", bearer.as_str()),
            ];
            let reply = kortex_platform::native::http(HttpRequest {
                method: "GET",
                url: &url,
                headers: &headers,
                body: None,
                timeout: Duration::from_secs(20),
                max_bytes: 4096,
            });
            remote.complete(
                reply
                    .ok()
                    .filter(|r| r.status == 200)
                    .and_then(|r| serde_json::from_slice::<serde_json::Value>(&r.body).ok())
                    .and_then(|v| Credits::from_json(&v)),
            );
        })
        .ok()?;
        slot.wait().await.flatten()
    }
}

/// The account the app talks for, found at start-up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Account {
    Checking,
    /// No session (browser) or no device token (desktop).
    SignedOut,
    Ready {
        auth: Auth,
        email: String,
    },
    /// The account could not be reached.
    Unavailable,
}

pub use platform::{Stream, account, cancel, credits, start};

/// Asks the model and returns its complete turn (and the credits left), streaming live events.
///
/// # Errors
/// See [`collect`]; also a transport that could not start.
pub async fn ask(
    auth: &Auth,
    request: &Request,
    on_event: impl FnMut(&Event),
) -> Result<(Turn, Option<Credits>), AskError> {
    let mut stream = start(auth, request).map_err(|e| AskError { status: 0, code: e })?;
    collect(&mut stream, on_event).await
}
