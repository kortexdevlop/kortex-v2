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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    let name = file.name();
    let mime = file
        .content_type()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| mime_from_name(&name).to_owned());
    Ok(SelectedFile {
        name,
        mime,
        bytes: bytes.to_vec(),
    })
}

/// File extensions per MIME type, used by native pickers (they filter by extension) and to name
/// the type of a file picked natively. Kept small: engines still check files by signature.
const EXTENSIONS: &[(&str, &[&str])] = &[
    ("image/png", &["png"]),
    ("image/jpeg", &["jpg", "jpeg"]),
    ("image/gif", &["gif"]),
    ("image/webp", &["webp"]),
    ("image/bmp", &["bmp"]),
    ("image/tiff", &["tif", "tiff"]),
    ("image/avif", &["avif"]),
    ("image/heic", &["heic"]),
    ("image/svg+xml", &["svg"]),
    ("image/x-icon", &["ico"]),
    ("application/pdf", &["pdf"]),
    ("application/json", &["json"]),
    ("application/zip", &["zip"]),
    ("text/plain", &["txt", "text", "log"]),
    ("text/markdown", &["md", "markdown"]),
    ("text/csv", &["csv"]),
    ("text/tab-separated-values", &["tsv"]),
    ("text/html", &["html", "htm"]),
    ("text/css", &["css"]),
    ("text/javascript", &["js", "mjs"]),
    ("text/xml", &["xml"]),
    ("text/yaml", &["yaml", "yml"]),
    ("text/x-toml", &["toml"]),
    ("audio/mpeg", &["mp3"]),
    ("audio/wav", &["wav"]),
    ("audio/ogg", &["ogg", "oga", "opus"]),
    ("audio/flac", &["flac"]),
    ("audio/mp4", &["m4a", "aac"]),
    ("video/mp4", &["mp4", "m4v"]),
    ("video/webm", &["webm"]),
    ("video/quicktime", &["mov"]),
    ("video/x-matroska", &["mkv"]),
];

/// Extensions a native picker should offer for an `accept` list of MIME patterns
/// (`image/*,application/pdf`). `None` means every file: an empty list, `*/*`, or a type we cannot
/// map. A native panel given an empty or unknown list greys out every file, or fails outright on
/// macOS, so this never returns an empty list.
#[must_use]
pub fn accept_extensions(accept: &str) -> Option<Vec<&'static str>> {
    let mut found: Vec<&'static str> = Vec::new();
    for pattern in accept.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let pattern = pattern.to_ascii_lowercase();
        if pattern == "*/*" || pattern == "*" {
            return None;
        }
        let matching: Vec<&'static str> = match pattern.strip_suffix("/*") {
            Some(family) => EXTENSIONS
                .iter()
                .filter(|(mime, _)| mime.split('/').next() == Some(family))
                .flat_map(|(_, ext)| ext.iter().copied())
                .collect(),
            None => EXTENSIONS
                .iter()
                .filter(|(mime, _)| *mime == pattern)
                .flat_map(|(_, ext)| ext.iter().copied())
                .collect(),
        };
        if matching.is_empty() {
            return None;
        }
        for ext in matching {
            if !found.contains(&ext) {
                found.push(ext);
            }
        }
    }
    (!found.is_empty()).then_some(found)
}

/// MIME type of a file from its name (`application/octet-stream` when unknown).
#[must_use]
pub fn mime_from_name(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    EXTENSIONS
        .iter()
        .find(|(_, exts)| exts.contains(&ext.as_str()))
        .map_or("application/octet-stream", |(mime, _)| mime)
}

/// Opens the system file dialog and reads the chosen files (desktop). The WebView's own
/// `<input type="file">` cannot offer "every file" on macOS, so native hosts use this instead.
/// Cancelling returns an empty list; an unreadable file is `file_read_failed`.
#[cfg(not(target_arch = "wasm32"))]
pub async fn pick_files(accept: &str, multiple: bool) -> crate::Result<Vec<SelectedFile>> {
    let mut dialog = rfd::AsyncFileDialog::new();
    if let Some(extensions) = accept_extensions(accept) {
        let label = extensions
            .iter()
            .map(|e| format!("*.{e}"))
            .collect::<Vec<_>>()
            .join(", ");
        dialog = dialog.add_filter(label, &extensions);
    }
    let handles = if multiple {
        dialog.pick_files().await.unwrap_or_default()
    } else {
        dialog.pick_file().await.into_iter().collect()
    };
    let paths: Vec<std::path::PathBuf> = handles.iter().map(|h| h.path().to_path_buf()).collect();
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    // Reading may take a while for large files: off the UI thread.
    let slot = crate::native::task::Slot::new();
    let remote = slot.clone();
    crate::native::task::spawn("kortex-pick-files", move || {
        remote.complete(read_paths(&paths));
    })
    .map_err(|_| crate::PlatformError("file_read_failed".into()))?;
    slot.wait()
        .await
        .unwrap_or_else(|| Err(crate::PlatformError("file_read_failed".into())))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_paths(paths: &[std::path::PathBuf]) -> crate::Result<Vec<SelectedFile>> {
    paths
        .iter()
        .map(|path| {
            let bytes = crate::native::fs::read(path)
                .map_err(|_| crate::PlatformError("file_read_failed".into()))?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(SelectedFile {
                mime: mime_from_name(&name).to_owned(),
                name,
                bytes,
            })
        })
        .collect()
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
/// Preferences shared by every Kortex host (site, apps, tools): theme, mode and text size.
/// On the web they live in one cookie valid for the whole parent domain (the only way to share
/// them across subdomains); natively they live in the shared preference store.
pub mod shared {
    /// Keys, their allowed values and the value meaning "unchanged".
    pub const KEYS: [(&str, &[&str], &str); 3] = [
        ("theme", &["light", "dark"], ""),
        ("mode", &["simple", "medium", "pro"], "medium"),
        ("size", &["large"], "normal"),
    ];
    pub const COOKIE: &str = "k-prefs";
    const SHORT: [(&str, &str); 3] = [("theme", "t"), ("mode", "m"), ("size", "s")];

    fn short(key: &str) -> Option<&'static str> {
        SHORT.iter().find(|(k, _)| *k == key).map(|(_, s)| *s)
    }
    fn allowed(key: &str, value: &str) -> bool {
        KEYS.iter()
            .any(|(k, values, _)| *k == key && values.contains(&value))
    }

    /// Value of `key` inside a cookie payload such as `t=dark&m=pro` (unknown values are ignored).
    #[must_use]
    pub fn parse(payload: &str, key: &str) -> Option<String> {
        let wanted = short(key)?;
        payload
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(k, _)| *k == wanted)
            .map(|(_, v)| v.to_owned())
            .filter(|v| allowed(key, v))
    }

    /// Payload with `key` set to `value`; a default or unknown value removes the key.
    #[must_use]
    pub fn merge(payload: &str, key: &str, value: &str) -> String {
        let Some(wanted) = short(key) else {
            return payload.to_owned();
        };
        let keep = allowed(key, value)
            && KEYS
                .iter()
                .any(|(k, _, default)| *k == key && *default != value);
        let mut pairs: Vec<String> = payload
            .split('&')
            .filter(|pair| {
                pair.split_once('=').is_some_and(|(k, v)| {
                    k != wanted && SHORT.iter().any(|(name, s)| *s == k && allowed(name, v))
                })
            })
            .map(str::to_owned)
            .collect();
        if keep {
            pairs.push(format!("{wanted}={value}"));
        }
        pairs.sort();
        pairs.join("&")
    }

    /// `Set-Cookie` text for `payload` (`Domain` only when the page is served from `domain`).
    #[must_use]
    pub fn cookie_text(payload: &str, host: &str, domain: &str, secure: bool) -> String {
        let scope =
            if !domain.is_empty() && (host == domain || host.ends_with(&format!(".{domain}"))) {
                format!("; Domain={domain}")
            } else {
                String::new()
            };
        let age = if payload.is_empty() { 0 } else { 31_536_000 };
        format!(
            "{COOKIE}={payload}; Path=/; Max-Age={age}; SameSite=Lax{scope}{}",
            if secure { "; Secure" } else { "" }
        )
    }
}

#[cfg(target_arch = "wasm32")]
fn browser_field(owner: &wasm_bindgen::JsValue, name: &str) -> wasm_bindgen::JsValue {
    use wasm_bindgen::JsValue;
    js_sys::Reflect::get(owner, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
}
#[cfg(target_arch = "wasm32")]
fn cookie_payload() -> String {
    let document = browser_field(&js_sys::global(), "document");
    let all = browser_field(&document, "cookie")
        .as_string()
        .unwrap_or_default();
    all.split(';')
        .filter_map(|c| c.trim().split_once('='))
        .find(|(name, _)| *name == shared::COOKIE)
        .map(|(_, v)| v.to_owned())
        .unwrap_or_default()
}

/// Shared preference (`theme`, `mode`, `size`); `None` when the person never chose one.
pub fn shared_get(key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        shared::parse(&cookie_payload(), key)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        preference_get("shared", key).ok().flatten().filter(|v| {
            shared::KEYS
                .iter()
                .any(|(k, values, _)| *k == key && values.contains(&v.as_str()))
        })
    }
}
/// Stores a shared preference after an explicit choice. `domain` is the parent domain the
/// cookie is shared on; a default value removes the preference (and the cookie when it is empty).
pub fn shared_set(domain: &str, key: &str, value: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsValue;
        let payload = shared::merge(&cookie_payload(), key, value);
        let global = js_sys::global();
        let location = browser_field(&global, "location");
        let host = browser_field(&location, "hostname")
            .as_string()
            .unwrap_or_default();
        let secure = browser_field(&location, "protocol").as_string().as_deref() == Some("https:");
        let text = shared::cookie_text(&payload, &host, domain, secure);
        let _ = js_sys::Reflect::set(
            &browser_field(&global, "document"),
            &JsValue::from_str("cookie"),
            &JsValue::from_str(&text),
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = domain;
        let kept = shared::merge("", key, value);
        let _ = preference_set("shared", key, if kept.is_empty() { "" } else { value });
    }
}
/// Forgets every shared preference.
pub fn shared_clear(domain: &str) {
    for (key, _, default) in shared::KEYS {
        shared_set(domain, key, default);
    }
}
pub fn apply_size(size: &str) {
    let large = size == "large";
    eval(&format!(
        "if({large}){{document.documentElement.dataset.size='large';}}else{{delete document.documentElement.dataset.size;}}"
    ));
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
      element?.contentWindow?.postMessage({{type:'kortex:init',size:document.documentElement.dataset.size||'normal',...init}},origin);"#
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
mod picker_tests {
    use super::{accept_extensions, mime_from_name};

    #[test]
    fn every_file_means_no_filter_never_an_empty_one() {
        for accept in [
            "",
            "*/*",
            " * ",
            "application/x-unknown",
            "image/*,application/x-unknown",
        ] {
            assert_eq!(accept_extensions(accept), None, "{accept:?}");
        }
    }

    #[test]
    fn mime_patterns_become_extensions() {
        assert_eq!(accept_extensions("application/pdf"), Some(vec!["pdf"]));
        let images = accept_extensions("image/*").unwrap_or_default();
        assert!(images.contains(&"png") && images.contains(&"jpeg") && images.contains(&"svg"));
        let text = accept_extensions("text/*, application/pdf").unwrap_or_default();
        assert!(text.contains(&"txt") && text.contains(&"csv") && text.ends_with(&["pdf"]));
        assert_eq!(accept_extensions("IMAGE/PNG,image/png"), Some(vec!["png"]));
    }

    #[test]
    fn names_give_their_type() {
        assert_eq!(mime_from_name("Rapport.PDF"), "application/pdf");
        assert_eq!(mime_from_name("photo.jpeg"), "image/jpeg");
        assert_eq!(mime_from_name("notes"), "application/octet-stream");
        assert_eq!(
            mime_from_name("archive.tar.zst"),
            "application/octet-stream"
        );
    }
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

#[cfg(test)]
mod shared_tests {
    use super::shared::{cookie_text, merge, parse};

    #[test]
    fn parse_reads_known_values_only() {
        assert_eq!(
            parse("t=dark&m=pro&s=large", "theme").as_deref(),
            Some("dark")
        );
        assert_eq!(
            parse("t=dark&m=pro&s=large", "mode").as_deref(),
            Some("pro")
        );
        assert_eq!(
            parse("t=dark&m=pro&s=large", "size").as_deref(),
            Some("large")
        );
        assert_eq!(parse("t=neon", "theme"), None);
        assert_eq!(parse("", "mode"), None);
        assert_eq!(parse("t=dark", "nope"), None);
    }

    #[test]
    fn merge_keeps_valid_pairs_and_drops_defaults() {
        assert_eq!(merge("", "theme", "dark"), "t=dark");
        assert_eq!(merge("t=dark", "mode", "pro"), "m=pro&t=dark");
        assert_eq!(merge("t=dark&m=pro", "theme", ""), "m=pro");
        assert_eq!(merge("t=dark&m=pro", "mode", "medium"), "t=dark");
        assert_eq!(
            merge("t=dark&x=1&m=weird", "size", "large"),
            "s=large&t=dark"
        );
        assert_eq!(merge("t=dark", "theme", ""), "");
    }

    #[test]
    fn cookie_is_scoped_and_expires_when_empty() {
        let text = cookie_text("t=dark", "flow.kortex.site", "kortex.site", true);
        assert_eq!(
            text,
            "k-prefs=t=dark; Path=/; Max-Age=31536000; SameSite=Lax; Domain=kortex.site; Secure"
        );
        let local = cookie_text("t=dark", "localhost", "kortex.site", false);
        assert!(!local.contains("Domain") && !local.contains("Secure"));
        assert!(cookie_text("", "kortex.site", "kortex.site", true).contains("Max-Age=0"));
    }
}
