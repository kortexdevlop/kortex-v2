//! Public documents (signed contract catalogue, release index): cookie-less fetch and a bounded
//! local cache. The caller verifies every document's signature, also when read back from the cache.
use crate::{PlatformError, Result};

/// Largest public document kept or fetched.
pub const MAX_BYTES: usize = 4 << 20;

fn name_ok(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_')
        && !name.starts_with('.');
    if ok {
        Ok(())
    } else {
        Err(PlatformError("storage_invalid_key".into()))
    }
}

/// GET `url` without cookies or credentials and returns its UTF-8 text (at most `max_bytes`).
///
/// # Errors
/// `network_failed` (transport, non-2xx status, redirect) or `response_too_large`.
pub async fn fetch_text(url: &str, max_bytes: usize) -> Result<String> {
    let max = max_bytes.min(MAX_BYTES);
    #[cfg(target_arch = "wasm32")]
    {
        web::fetch_text(url, max).await
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::native::task::{Slot, spawn};
        let slot = Slot::new();
        let remote = slot.clone();
        let url = url.to_owned();
        spawn("kortex-public-fetch", move || {
            let response = crate::native::http(crate::native::HttpRequest {
                method: "GET",
                url: &url,
                headers: &[],
                body: None,
                timeout: std::time::Duration::from_secs(20),
                max_bytes: u64::try_from(max).unwrap_or(u64::MAX),
            });
            remote.complete(match response {
                Ok(r) if (200..300).contains(&r.status) => {
                    String::from_utf8(r.body).map_err(|_| PlatformError("network_failed".into()))
                }
                _ => Err(PlatformError("network_failed".into())),
            });
        })
        .map_err(|_| PlatformError("network_failed".into()))?;
        slot.wait()
            .await
            .unwrap_or_else(|| Err(PlatformError("network_failed".into())))
    }
}

/// Last cached copy of a public document, if any (unverified: check its signature).
#[must_use]
pub fn cache_get(name: &str) -> Option<String> {
    name_ok(name).ok()?;
    #[cfg(target_arch = "wasm32")]
    {
        web::get(name)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let bytes = crate::native::fs::read(native_dir()?.join(name)).ok()?;
        (bytes.len() <= MAX_BYTES).then_some(())?;
        String::from_utf8(bytes).ok()
    }
}

/// Stores a public document (replaced atomically on desktop).
///
/// # Errors
/// `storage_invalid_key`, `storage_value_too_large` or `storage_unavailable`.
pub fn cache_put(name: &str, text: &str) -> Result<()> {
    name_ok(name)?;
    if text.len() > MAX_BYTES {
        return Err(PlatformError("storage_value_too_large".into()));
    }
    #[cfg(target_arch = "wasm32")]
    {
        web::set(name, text)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let unavailable = |_| PlatformError("storage_unavailable".into());
        let dir = native_dir().ok_or_else(|| PlatformError("storage_unavailable".into()))?;
        crate::native::fs::create_dir_all(&dir).map_err(unavailable)?;
        let tmp = dir.join(format!(".{name}.{}.tmp", crate::native::process_id()));
        crate::native::fs::write(&tmp, text.as_bytes()).map_err(unavailable)?;
        crate::native::fs::rename(&tmp, dir.join(name)).map_err(unavailable)
    }
}

/// `$KORTEX_HOME/cache/public` (default `~/.kortex`), shared by the apps and the CLI.
#[cfg(not(target_arch = "wasm32"))]
fn native_dir() -> Option<std::path::PathBuf> {
    crate::native::env::var_os("KORTEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| crate::native::env::home_dir().map(|p| p.join(".kortex")))
        .map(|root| root.join("cache").join("public"))
}

#[cfg(target_arch = "wasm32")]
mod web {
    use crate::{PlatformError, Result};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    fn failed() -> PlatformError {
        PlatformError("network_failed".into())
    }

    fn get_fn(target: &JsValue, name: &str) -> Option<js_sys::Function> {
        js_sys::Reflect::get(target, &JsValue::from_str(name))
            .ok()?
            .dyn_into::<js_sys::Function>()
            .ok()
    }

    pub async fn fetch_text(url: &str, max: usize) -> Result<String> {
        let global = js_sys::global();
        let fetch = get_fn(&global, "fetch").ok_or_else(failed)?;
        let init = js_sys::Object::new();
        for (key, value) in [
            ("credentials", "omit"),
            ("cache", "no-cache"),
            ("redirect", "error"),
            ("mode", "cors"),
        ] {
            js_sys::Reflect::set(&init, &JsValue::from_str(key), &JsValue::from_str(value))
                .map_err(|_| failed())?;
        }
        let promise = fetch
            .call2(&global, &JsValue::from_str(url), &init)
            .map_err(|_| failed())?;
        let response = JsFuture::from(js_sys::Promise::from(promise))
            .await
            .map_err(|_| failed())?;
        let ok = js_sys::Reflect::get(&response, &JsValue::from_str("ok"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !ok {
            return Err(failed());
        }
        let buffer = get_fn(&response, "arrayBuffer")
            .ok_or_else(failed)?
            .call0(&response)
            .map_err(|_| failed())?;
        let buffer = JsFuture::from(js_sys::Promise::from(buffer))
            .await
            .map_err(|_| failed())?;
        let bytes = js_sys::Uint8Array::new(&buffer);
        if bytes.length() as usize > max {
            return Err(PlatformError("response_too_large".into()));
        }
        String::from_utf8(bytes.to_vec()).map_err(|_| failed())
    }

    fn storage() -> Option<JsValue> {
        js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("localStorage"))
            .ok()
            .filter(|s| !s.is_null() && !s.is_undefined())
    }

    fn key(name: &str) -> JsValue {
        JsValue::from_str(&format!("kortex:public:v1:{name}"))
    }

    pub fn get(name: &str) -> Option<String> {
        let storage = storage()?;
        get_fn(&storage, "getItem")?
            .call1(&storage, &key(name))
            .ok()?
            .as_string()
    }

    pub fn set(name: &str, text: &str) -> Result<()> {
        let storage = storage().ok_or_else(|| PlatformError("storage_unavailable".into()))?;
        get_fn(&storage, "setItem")
            .ok_or_else(|| PlatformError("storage_unavailable".into()))?
            .call2(&storage, &key(name), &JsValue::from_str(text))
            .map(|_| ())
            .map_err(|_| PlatformError("storage_operation_failed".into()))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn names_cannot_escape_the_cache() {
        for bad in ["", "../x", ".hidden", "a/b", "a\\b", &"x".repeat(65)] {
            assert!(name_ok(bad).is_err(), "{bad:?}");
        }
        assert!(name_ok("catalog-stable.json").is_ok());
    }
}
