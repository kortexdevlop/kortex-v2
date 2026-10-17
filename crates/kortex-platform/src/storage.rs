//! Small, non-secret preferences. The namespace is isolation by convention, not a sandbox.
use crate::{PlatformError, Result};

pub const MAX_VALUE_BYTES: usize = 64 * 1024;
const MAX_COMPONENT_BYTES: usize = 120;

fn component(value: &str) -> Result<String> {
    if value.is_empty() || value.len() > MAX_COMPONENT_BYTES {
        return Err(PlatformError("storage_invalid_key".into()));
    }
    // Injective UTF-8 encoding: separators and Unicode can never escape a namespace.
    Ok(value
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
fn validate_value(value: &str) -> Result<()> {
    if value.len() > MAX_VALUE_BYTES {
        return Err(PlatformError("storage_value_too_large".into()));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use super::*;
    use crate::Storage;
    use std::{
        io::Write,
        path::{Path, PathBuf},
    };

    /// A caller-selected application data directory. Never store credentials here.
    pub struct NativeStorage {
        directory: PathBuf,
    }
    impl NativeStorage {
        pub fn open(root: impl AsRef<Path>, namespace: &str) -> Result<Self> {
            let directory = root.as_ref().join(component(namespace)?);
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder
                .create(&directory)
                .map_err(|_| PlatformError("storage_unavailable".into()))?;
            if !std::fs::symlink_metadata(&directory)
                .map_err(|_| PlatformError("storage_unavailable".into()))?
                .is_dir()
            {
                return Err(PlatformError("storage_unavailable".into()));
            }
            Ok(Self { directory })
        }
        fn path(&self, key: &str) -> Result<PathBuf> {
            Ok(self.directory.join(component(key)?))
        }
    }
    impl Storage for NativeStorage {
        fn get(&self, key: &str) -> Result<Option<String>> {
            let path = self.path(key)?;
            match std::fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Ok(meta) if meta.is_file() => (),
                _ => return Err(PlatformError("storage_read_failed".into())),
            }
            let bytes = crate::native::fs::read_limited(path, MAX_VALUE_BYTES as u64)
                .map_err(|_| PlatformError("storage_read_failed".into()))?;
            String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| PlatformError("storage_read_failed".into()))
        }
        fn set(&mut self, key: &str, value: &str) -> Result<()> {
            let path = self.path(key)?;
            validate_value(value)?;
            let mut random = [0; 16];
            crate::fill_random(&mut random)?;
            let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
            let temporary = self.directory.join(format!(".pending-{suffix}"));
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temporary)
                .map_err(|_| PlatformError("storage_write_failed".into()))?;
            let result = (|| {
                file.write_all(value.as_bytes())?;
                file.sync_all()?;
                drop(file);
                std::fs::rename(&temporary, &path)
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&temporary);
                return Err(PlatformError("storage_write_failed".into()));
            }
            Ok(())
        }
        fn remove(&mut self, key: &str) -> Result<()> {
            match std::fs::remove_file(self.path(key)?) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(_) => Err(PlatformError("storage_write_failed".into())),
            }
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use desktop::NativeStorage;

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use crate::Storage;
    use wasm_bindgen::{JsCast, JsValue};

    /// Origin-local browser preferences. Access denied/quota errors stay explicit.
    pub struct WebStorage {
        storage: JsValue,
        prefix: String,
    }
    impl WebStorage {
        pub fn open(namespace: &str) -> Result<Self> {
            let prefix = format!("kortex:prefs:v1:{}:", component(namespace)?);
            let storage =
                js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("localStorage"))
                    .map_err(|_| PlatformError("storage_unavailable".into()))?;
            if storage.is_null() || storage.is_undefined() {
                return Err(PlatformError("storage_unavailable".into()));
            }
            Ok(Self { storage, prefix })
        }
        fn key(&self, key: &str) -> Result<JsValue> {
            Ok(JsValue::from_str(&format!(
                "{}{}",
                self.prefix,
                component(key)?
            )))
        }
        fn call(&self, method: &str, args: &js_sys::Array) -> Result<JsValue> {
            let function = js_sys::Reflect::get(&self.storage, &JsValue::from_str(method))
                .ok()
                .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
                .ok_or_else(|| PlatformError("storage_unavailable".into()))?;
            function
                .apply(&self.storage, args)
                .map_err(|_| PlatformError("storage_operation_failed".into()))
        }
    }
    impl Storage for WebStorage {
        fn get(&self, key: &str) -> Result<Option<String>> {
            let value = self.call("getItem", &js_sys::Array::of1(&self.key(key)?))?;
            if value.is_null() {
                return Ok(None);
            }
            let value = value
                .as_string()
                .ok_or_else(|| PlatformError("storage_read_failed".into()))?;
            validate_value(&value)?;
            Ok(Some(value))
        }
        fn set(&mut self, key: &str, value: &str) -> Result<()> {
            validate_value(value)?;
            self.call(
                "setItem",
                &js_sys::Array::of2(&self.key(key)?, &JsValue::from_str(value)),
            )?;
            Ok(())
        }
        fn remove(&mut self, key: &str) -> Result<()> {
            self.call("removeItem", &js_sys::Array::of1(&self.key(key)?))?;
            Ok(())
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub use browser::WebStorage;
