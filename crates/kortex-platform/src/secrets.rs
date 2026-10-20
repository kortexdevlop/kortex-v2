//! OS-protected native credentials. No file fallback and no backend error payloads in logs.
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretError {
    Unavailable,
    LockedOrDenied,
    Corrupt,
    InvalidInput,
    Backend,
}

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "system credential store unavailable",
            Self::LockedOrDenied => "system credential store locked or access denied",
            Self::Corrupt => "invalid protected credential record",
            Self::InvalidInput => "invalid credential store request",
            Self::Backend => "system credential store operation failed",
        })
    }
}
impl std::error::Error for SecretError {}

pub trait SecretStore: fmt::Debug + Send + Sync {
    fn read(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>, SecretError>;
    fn write(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), SecretError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError>;
}

#[derive(Debug, Default)]
pub struct NativeSecretStore;

fn safe_error(error: keyring::v1::Error) -> SecretError {
    use keyring::v1::Error;
    match error {
        Error::NoStorageAccess(_) => SecretError::LockedOrDenied,
        Error::BadEncoding(_)
        | Error::BadDataFormat(_, _)
        | Error::BadStoreFormat(_)
        | Error::Ambiguous(_) => SecretError::Corrupt,
        Error::Invalid(_, _) | Error::TooLong(_, _) => SecretError::InvalidInput,
        Error::NoDefaultStore | Error::NotSupportedByStore(_) => SecretError::Unavailable,
        _ => SecretError::Backend,
    }
}
impl SecretStore for NativeSecretStore {
    fn read(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>, SecretError> {
        let entry = keyring::v1::Entry::new(service, account).map_err(safe_error)?;
        match entry.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::v1::Error::NoEntry) => Ok(None),
            Err(error) => Err(safe_error(error)),
        }
    }
    fn write(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), SecretError> {
        keyring::v1::Entry::new(service, account)
            .map_err(safe_error)?
            .set_secret(secret)
            .map_err(safe_error)
    }
    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError> {
        let entry = keyring::v1::Entry::new(service, account).map_err(safe_error)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::v1::Error::NoEntry) => Ok(()),
            Err(error) => Err(safe_error(error)),
        }
    }
}

/// Explicitly injected by tests; never selected automatically by production code.
#[derive(Default)]
pub struct MemorySecretStore(Mutex<BTreeMap<(String, String), Vec<u8>>>);
impl fmt::Debug for MemorySecretStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MemorySecretStore([redacted])")
    }
}
impl SecretStore for MemorySecretStore {
    fn read(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>, SecretError> {
        Ok(self
            .0
            .lock()
            .map_err(|_| SecretError::Backend)?
            .get(&(service.into(), account.into()))
            .cloned())
    }
    fn write(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), SecretError> {
        self.0
            .lock()
            .map_err(|_| SecretError::Backend)?
            .insert((service.into(), account.into()), secret.to_vec());
        Ok(())
    }
    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError> {
        self.0
            .lock()
            .map_err(|_| SecretError::Backend)?
            .remove(&(service.into(), account.into()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backend_payloads_and_memory_debug_never_expose_secrets() {
        let secret = b"private-device-token";
        let store = MemorySecretStore::default();
        store.write("service", "user", secret).expect("write");
        assert!(!format!("{store:?}").contains("private-device-token"));
        let error = safe_error(keyring::v1::Error::BadEncoding(secret.to_vec()));
        assert_eq!(error, SecretError::Corrupt);
        assert!(!format!("{error:?}: {error}").contains("private-device-token"));
        assert_eq!(store.read("other", "user").expect("read"), None);
    }
    #[test]
    #[ignore = "requires an isolated OS credential store; run explicitly in native CI"]
    fn native_secret_roundtrip() {
        assert_eq!(
            std::env::var("KORTEX_SECRET_STORE_QA").as_deref(),
            Ok("isolated")
        );
        let account = format!("qa-{}-{}", std::process::id(), crate::now_ms());
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = NativeSecretStore.delete("site.kortex.qa", &self.0);
            }
        }
        let _cleanup = Cleanup(account.clone());
        let store = NativeSecretStore;
        assert_eq!(
            store
                .read("site.kortex.qa", &account)
                .expect("initial read"),
            None
        );
        store
            .write("site.kortex.qa", &account, b"public-test-fixture")
            .expect("protected write");
        assert_eq!(
            store
                .read("site.kortex.qa", &account)
                .expect("protected read"),
            Some(b"public-test-fixture".to_vec())
        );
        store
            .delete("site.kortex.qa", &account)
            .expect("protected deletion");
        assert_eq!(
            store
                .read("site.kortex.qa", &account)
                .expect("deleted read"),
            None
        );
    }
}
