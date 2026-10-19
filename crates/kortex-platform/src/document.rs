//! Browser/WebView capabilities. UI code never builds or evaluates platform scripts.
use dioxus::document::{Eval, eval};

pub fn host_bus() -> Eval {
    eval(include_str!("host-bus.js"))
}
pub fn launch_query() -> Eval {
    eval(
        "const q = new URLSearchParams(location.search); dioxus.send((q.get('lang') || 'fr') + '|' + (q.get('variant') || '') + '|' + (q.get('mode') || ''))",
    )
}

/// Waits `ms` milliseconds on the page's own timer (browser and WebView alike).
pub async fn sleep(ms: u32) {
    let _ = eval(&format!(
        "await new Promise(r => setTimeout(r, {ms})); return 1;"
    ))
    .await;
}

/// Minutes east of UTC on this device (the page's own clock settings, browser and WebView alike).
pub async fn utc_offset_minutes() -> i32 {
    eval("return -new Date().getTimezoneOffset();")
        .join::<i32>()
        .await
        .unwrap_or(0)
}

/// Standard base64 for previews and binary downloads.
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                char::from(T[((n >> (18 - 6 * i)) & 63) as usize])
            } else {
                '='
            });
        }
    }
    out
}
fn download_script(name: &str, mime: &str, bytes: &[u8]) -> String {
    let mime = if mime.is_empty() {
        "application/octet-stream"
    } else {
        mime
    };
    let args = serde_json::json!([base64(bytes), mime, name]);
    include_str!("download.js").replace("__KORTEX_ARGS__", &args.to_string())
}
pub fn download(name: &str, mime: &str, bytes: &[u8]) {
    eval(&download_script(name, mime, bytes));
}
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
pub(crate) fn clipboard_operation(
    text: Option<&str>,
) -> impl std::future::Future<Output = crate::Result<String>> + use<> {
    let script = format!(
        r#"
        try {{
            const text = {};
            if (!navigator.clipboard) return {{ error: "clipboard_unavailable" }};
            if (text === null) return {{ value: await navigator.clipboard.readText() }};
            await navigator.clipboard.writeText(text);
            return {{ value: "" }};
        }} catch (error) {{
            return {{ error: ["NotAllowedError", "SecurityError"].includes(error?.name) ? "clipboard_denied" : "clipboard_failed" }};
        }}
    "#,
        serde_json::json!(text)
    );
    let operation = eval(&script);
    async move {
        let reply = operation
            .join::<serde_json::Value>()
            .await
            .map_err(|_| crate::PlatformError("clipboard_failed".into()))?;
        if let Some(value) = reply.get("value").and_then(serde_json::Value::as_str) {
            return Ok(value.to_owned());
        }
        let code = match reply.get("error").and_then(serde_json::Value::as_str) {
            Some("clipboard_unavailable") => "clipboard_unavailable",
            Some("clipboard_denied") => "clipboard_denied",
            _ => "clipboard_failed",
        };
        Err(crate::PlatformError(code.into()))
    }
}

/// A file explicitly selected by the user in a browser or native WebView.
pub struct SelectedFile {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}
pub async fn read_selected(file: &dioxus::html::FileData) -> crate::Result<SelectedFile> {
    let bytes = file
        .read_bytes()
        .await
        .map_err(|_| crate::PlatformError("file_read_failed".into()))?;
    Ok(SelectedFile {
        name: file.name(),
        mime: file.content_type().unwrap_or_default(),
        bytes: bytes.to_vec(),
    })
}

/// Non-secret app preferences; backends share the same Storage contract.
pub fn preference_get(namespace: &str, key: &str) -> crate::Result<Option<String>> {
    use crate::Storage;
    preference_store(namespace)?.get(key)
}
pub fn preference_set(namespace: &str, key: &str, value: &str) -> crate::Result<()> {
    use crate::Storage;
    preference_store(namespace)?.set(key, value)
}
#[cfg(target_arch = "wasm32")]
fn preference_store(namespace: &str) -> crate::Result<crate::storage::WebStorage> {
    crate::storage::WebStorage::open(namespace)
}
#[cfg(not(target_arch = "wasm32"))]
fn preference_store(namespace: &str) -> crate::Result<crate::storage::NativeStorage> {
    let root = crate::native::env::var_os("KORTEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| crate::native::env::home_dir().map(|p| p.join(".kortex")))
        .ok_or_else(|| crate::PlatformError("storage_unavailable".into()))?;
    crate::storage::NativeStorage::open(root.join("preferences"), namespace)
}
pub fn initial_language() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsValue;
        let location = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("location")).ok();
        let field = |key: &str| {
            location
                .as_ref()
                .and_then(|l| js_sys::Reflect::get(l, &JsValue::from_str(key)).ok())
                .and_then(|v| v.as_string())
                .unwrap_or_default()
        };
        if field("search").contains("lang=en") || field("pathname").starts_with("/en/") {
            return "en".into();
        }
    }
    "fr".into()
}
pub fn apply_theme(theme: &str) {
    let theme = if ["light", "dark"].contains(&theme) {
        theme
    } else {
        ""
    };
    eval(&format!(
        "document.documentElement.dataset.theme={};",
        serde_json::json!(theme)
    ));
}
pub fn apply_language(lang: &str) {
    let lang = if lang == "en" { "en" } else { "fr" };
    eval(&format!(
        "document.documentElement.lang={};",
        serde_json::json!(lang)
    ));
}
pub fn tool_host_events(frame: u64, origin: &str) -> Eval {
    eval(&include_str!("app-host.js").replace(
        "__KORTEX_ARGS__",
        &serde_json::json!([frame, origin]).to_string(),
    ))
}
pub fn tool_host_init(frame: u64, origin: &str, init: &impl serde::Serialize) {
    let args = serde_json::json!([frame, origin, init]);
    eval(&format!(
        r#"const [frame, origin, init] = {args};
      const element=Array.from(document.querySelectorAll('iframe[data-kortex-window]')).find(e=>e.dataset.kortexWindow===String(frame));
      element?.contentWindow?.postMessage({{type:'kortex:init',...init}},origin);"#
    ));
}
pub async fn element_size(element: &dioxus::html::MountedData) -> crate::Result<(f64, f64)> {
    element
        .get_client_rect()
        .await
        .map(|r| (r.size.width, r.size.height))
        .map_err(|_| crate::PlatformError("viewport_unavailable".into()))
}
pub fn launch_tool_id() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsValue;
        let location =
            js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("location")).ok()?;
        let query = js_sys::Reflect::get(&location, &JsValue::from_str("search"))
            .ok()?
            .as_string()?;
        return query
            .trim_start_matches('?')
            .split('&')
            .find_map(|p| p.strip_prefix("tool=").map(str::to_owned));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

/// Cookie-authenticated JSON delivery. No session cookie or token is exposed to Rust/UI storage.
#[derive(serde::Deserialize)]
pub struct AccountResponse {
    pub status: u16,
    pub body: String,
}
pub fn cookie_session_supported() -> bool {
    cfg!(target_arch = "wasm32")
}
pub async fn account_request(
    url: &str,
    method: &str,
    body: Option<&str>,
    user: Option<&str>,
) -> crate::Result<AccountResponse> {
    if !cookie_session_supported() {
        return Err(crate::PlatformError("account_native_unavailable".into()));
    }
    if !(url.starts_with("https://") || is_dev_origin(url))
        || !["GET", "PUT"].contains(&method)
        || body.is_some_and(|s| s.len() > 70_000)
    {
        return Err(crate::PlatformError("account_invalid_request".into()));
    }
    let args = serde_json::json!([url, method, body, 131_072, user]);
    let reply =
        eval(&include_str!("account-fetch.js").replace("__KORTEX_ARGS__", &args.to_string()))
            .join::<AccountResponse>()
            .await
            .map_err(|_| crate::PlatformError("account_network".into()))?;
    if reply.status == 0 {
        return Err(crate::PlatformError("account_network".into()));
    }
    Ok(reply)
}

/// Sends one HTTP request through the account API's proxy (`POST {api}/v1/proxy`, session cookie,
/// signed-in users only). `payload` is the proxy's JSON request; the reply is its JSON envelope.
/// The cookie never reaches Rust or UI storage.
///
/// # Errors
/// `proxy_invalid_request` (not the proxy route, or a payload over 300 KB) or `proxy_network`.
pub async fn proxy_request(api: &str, payload: &str) -> crate::Result<AccountResponse> {
    let url = format!("{api}/v1/proxy");
    if !cookie_session_supported() {
        return Err(crate::PlatformError("proxy_native_unavailable".into()));
    }
    if !(url.starts_with("https://") || is_dev_origin(&url)) || payload.len() > 300_000 {
        return Err(crate::PlatformError("proxy_invalid_request".into()));
    }
    let args = serde_json::json!([url, "POST", payload, 3_500_000, serde_json::Value::Null]);
    let reply =
        eval(&include_str!("account-fetch.js").replace("__KORTEX_ARGS__", &args.to_string()))
            .join::<AccountResponse>()
            .await
            .map_err(|_| crate::PlatformError("proxy_network".into()))?;
    if reply.status == 0 {
        return Err(crate::PlatformError("proxy_network".into()));
    }
    Ok(reply)
}

/// One step of a streamed `fetch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchEvent {
    /// The response status, sent once before any chunk.
    Status(u16),
    /// A piece of the response body, decoded as UTF-8.
    Chunk(String),
    End,
    /// Network error, abort or oversized body.
    Failed,
}

#[derive(serde::Deserialize)]
struct RawFetchEvent {
    status: Option<u16>,
    chunk: Option<String>,
    end: Option<bool>,
}

/// A cookie-authenticated POST whose response body is read as it arrives (server-sent events).
/// The session cookie never reaches Rust or UI storage.
pub struct FetchStream(Eval);

impl FetchStream {
    /// Next event; after `End` or `Failed` keep calling and it returns `Failed`.
    pub async fn next(&mut self) -> FetchEvent {
        match self.0.recv::<RawFetchEvent>().await {
            Ok(RawFetchEvent {
                status: Some(status),
                ..
            }) => FetchEvent::Status(status),
            Ok(RawFetchEvent {
                chunk: Some(chunk), ..
            }) => FetchEvent::Chunk(chunk),
            Ok(RawFetchEvent {
                end: Some(true), ..
            }) => FetchEvent::End,
            _ => FetchEvent::Failed,
        }
    }

    /// Aborts the request; the stream then ends with `Failed`.
    pub fn cancel(&self) {
        let _ = self.0.send(true);
    }

    /// A copyable handle that cancels this request from elsewhere (a Stop button) while another
    /// task is waiting in [`Self::next`].
    #[must_use]
    pub fn canceller(&self) -> FetchCanceller {
        FetchCanceller(self.0)
    }
}

/// Cancels a [`FetchStream`] it was taken from.
#[derive(Clone, Copy)]
pub struct FetchCanceller(Eval);

impl FetchCanceller {
    pub fn cancel(&self) {
        let _ = self.0.send(true);
    }
}

/// A development API base given in the page URL (`?api=http://127.0.0.1:8787`), honoured only when
/// the page itself is served from `localhost` or `127.0.0.1`; `None` otherwise (production).
pub async fn dev_api_override() -> Option<String> {
    let value = eval(
        "const q = new URLSearchParams(location.search).get('api') || ''; \
         const local = ['localhost', '127.0.0.1'].includes(location.hostname); \
         return local && /^http:\\/\\/(localhost|127\\.0\\.0\\.1)(:[0-9]+)?$/.test(q) ? q : '';",
    )
    .join::<String>()
    .await
    .ok()?;
    (!value.is_empty()).then_some(value)
}

fn is_dev_origin(url: &str) -> bool {
    ["http://localhost", "http://127.0.0.1"]
        .iter()
        .any(|prefix| {
            url.strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with([':', '/']))
        })
}

/// Starts a streamed POST. HTTPS only (plain HTTP to `localhost`/`127.0.0.1` for development).
///
/// # Errors
/// `fetch_native_unavailable` outside the browser, `fetch_invalid_request` for a refused URL or
/// an oversized body.
pub fn fetch_stream(url: &str, body: &str, user: Option<&str>) -> crate::Result<FetchStream> {
    if !cookie_session_supported() {
        return Err(crate::PlatformError("fetch_native_unavailable".into()));
    }
    if !(url.starts_with("https://") || is_dev_origin(url)) || body.len() > 170_000 {
        return Err(crate::PlatformError("fetch_invalid_request".into()));
    }
    let args = serde_json::json!([url, body, user]);
    Ok(FetchStream(eval(
        &include_str!("fetch-stream.js").replace("__KORTEX_ARGS__", &args.to_string()),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_fetch_accepts_https_and_local_dev_only() {
        assert!(is_dev_origin("http://localhost:8787/v1/ai/chat"));
        assert!(is_dev_origin("http://127.0.0.1/x"));
        assert!(!is_dev_origin("http://localhost.evil.test/x"));
        assert!(!is_dev_origin("http://example.com/x"));
        // Outside a browser the call is refused whatever the URL.
        assert!(fetch_stream("https://api.kortex.site/v1/ai/chat", "{}", None).is_err());
    }

    #[test]
    fn binary_and_untrusted_names_are_serialized_without_script_interpolation() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        let name = "quote\"\\\n\u{2028};throw new Error('injected')";
        let script = download_script(name, "", &[0, 255]);
        let args = script
            .split("const [data, mime, name] = ")
            .nth(1)
            .expect("args")
            .split(";\n")
            .next()
            .expect("end");
        let value: serde_json::Value = serde_json::from_str(args).expect("valid JSON");
        assert_eq!(
            value,
            serde_json::json!(["AP8=", "application/octet-stream", name])
        );
    }
}
