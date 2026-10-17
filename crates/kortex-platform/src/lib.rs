//! Platform abstraction. Tools never touch `web_sys` / `std::fs` directly (checked by xtask).
//! Implementations: `web`, `desktop` (added with kortex-ui), later `os`.
use std::collections::BTreeMap;

pub mod clipboard;
pub mod storage;

#[cfg(all(feature = "secret-store", not(target_arch = "wasm32")))]
pub mod secrets;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod offline;
#[cfg(target_arch = "wasm32")]
pub mod worker;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError(pub String);

pub type Result<T> = std::result::Result<T, PlatformError>;

/// Key-value storage (preferences, history).
pub trait Storage {
    /// # Errors
    /// Backend failure.
    fn get(&self, key: &str) -> Result<Option<String>>;
    /// # Errors
    /// Backend failure.
    fn set(&mut self, key: &str, value: &str) -> Result<()>;
    /// # Errors
    /// Backend failure.
    fn remove(&mut self, key: &str) -> Result<()>;
}

/// System clipboard.
pub trait Clipboard {
    /// # Errors
    /// Permission denied or unavailable.
    fn write_text(&mut self, text: &str) -> Result<()>;
    /// # Errors
    /// Permission denied or unavailable.
    fn read_text(&self) -> Result<String>;
}

/// In-memory implementation for tests and headless runs.
#[derive(Debug, Default)]
pub struct MemoryPlatform {
    store: BTreeMap<String, String>,
    clip: String,
}

impl Storage for MemoryPlatform {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.store.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: &str) -> Result<()> {
        self.store.insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<()> {
        self.store.remove(key);
        Ok(())
    }
}

impl Clipboard for MemoryPlatform {
    fn write_text(&mut self, text: &str) -> Result<()> {
        text.clone_into(&mut self.clip);
        Ok(())
    }

    fn read_text(&self) -> Result<String> {
        Ok(self.clip.clone())
    }
}

/// Owned transport error; independent of engine and UI types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerFailure {
    pub code: String,
    pub detail: Option<String>,
}

#[cfg(feature = "document")]
pub mod document;

/// Platform wall clock in UTC milliseconds. Deterministic engines inject a clock via Cx.
#[must_use]
pub fn now_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    }
}

/// Fill bytes using the operating system/browser CSPRNG, never a fallback PRNG.
pub fn fill_random(bytes: &mut [u8]) -> Result<()> {
    getrandom::fill(bytes).map_err(|_| PlatformError("entropy_unavailable".into()))
}

/// Yield during the synchronous headless executor's polling loop.
pub fn yield_now() {
    std::thread::yield_now();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_roundtrip() {
        let mut p = MemoryPlatform::default();
        assert_eq!(p.get("mode"), Ok(None));
        p.set("mode", "pro").unwrap_or_default();
        assert_eq!(p.get("mode"), Ok(Some("pro".to_owned())));
        p.remove("mode").unwrap_or_default();
        assert_eq!(p.get("mode"), Ok(None));
    }

    #[test]
    fn clipboard_roundtrip() {
        let mut p = MemoryPlatform::default();
        p.write_text("hi").unwrap_or_default();
        assert_eq!(p.read_text(), Ok("hi".to_owned()));
    }
}
