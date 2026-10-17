//! Account link: device token login, installs recorded on the account, password confirmation.
//!
//! The password never leaves the machine: it is turned into `auth_key` with the same slow KDF as the
//! website (`services/shared/src/auth.ts`, ADR 0005) and only that key is sent. Device tokens are
//! revocable from the account page and can never delete the account.
use std::time::Duration;

use hmac::Hmac;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::{Kind, PkgError, Result};
use crate::home::Home;
use crate::index::b64_encode;
use crate::net::is_local_host;

pub const DEFAULT_API: &str = "https://api.kortex.site";
pub const KDF_ITERATIONS: u32 = 600_000;
const KDF_VERSION: u32 = 1;

/// `auth_key = PBKDF2-SHA256(password, "kortex-auth-v1:" + lowercase(trim(email)), iterations)`,
/// base64url without padding. Must stay byte-identical to the JavaScript implementation.
#[must_use]
pub fn derive_auth_key(email: &str, password: &str, iterations: u32) -> String {
    let salt = format!("kortex-auth-v{KDF_VERSION}:{}", email.trim().to_lowercase());
    let mut out = [0_u8; 32];
    pbkdf2::pbkdf2::<Hmac<Sha256>>(password.as_bytes(), salt.as_bytes(), iterations, &mut out)
        .expect("hmac accepts any key length");
    b64_encode(&out)
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_owned()
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub api: String,
    pub email: String,
    pub device_id: String,
    pub token: String,
}

const CREDENTIAL_SERVICE: &str = "site.kortex.desktop";
const MAX_CREDENTIAL_BYTES: u64 = 8192;

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("api", &self.api)
            .field("email", &self.email)
            .field("device_id", &self.device_id)
            .field("token", &"[redacted]")
            .finish()
    }
}

fn credential_error() -> PkgError {
    PkgError::new(
        Kind::CredentialStore,
        "enregistrement de compte protégé invalide",
    )
}
fn store_error(error: kortex_platform::secrets::SecretError) -> PkgError {
    use kortex_platform::secrets::SecretError;
    let message = match error {
        SecretError::Unavailable => {
            "trousseau système indisponible : active le gestionnaire de secrets du système"
        }
        SecretError::LockedOrDenied => {
            "trousseau système verrouillé ou accès refusé : déverrouille-le et réessaie"
        }
        SecretError::Corrupt => "enregistrement protégé illisible dans le trousseau système",
        SecretError::InvalidInput => "requête de stockage protégé invalide",
        SecretError::Backend => "échec du trousseau système : réessaie sans installer de logiciel",
    };
    PkgError::new(Kind::CredentialStore, message)
}
fn parse_credentials(bytes: &[u8]) -> Result<Credentials> {
    if bytes.len() > MAX_CREDENTIAL_BYTES as usize {
        return Err(credential_error());
    }
    let c: Credentials = serde_json::from_slice(bytes).map_err(|_| credential_error())?;
    if !(c.api.starts_with("https://") || is_local_host(&c.api))
        || c.api.len() > 2048
        || c.api.chars().any(char::is_control)
        || c.email.len() > 320
        || !c.email.contains('@')
        || c.email.chars().any(char::is_control)
        || c.device_id.is_empty()
        || c.device_id.len() > 256
        || c.device_id.chars().any(char::is_control)
        || c.token.is_empty()
        || c.token.len() > 2048
        || !c.token.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(credential_error());
    }
    Ok(c)
}
impl Home {
    /// Legacy plaintext location, retained only for verified migration/deletion.
    #[must_use]
    pub fn credentials_path(&self) -> std::path::PathBuf {
        self.root().join("credentials.json")
    }

    fn credential_account(&self) -> Result<String> {
        kortex_platform::native::fs::create_dir_all(self.root())?;
        let path = kortex_platform::native::fs::canonicalize(self.root())?;
        Ok(format!(
            "v1:{}",
            crate::sha256_hex(path.as_os_str().as_encoded_bytes())
        ))
    }
    fn remove_legacy_credentials(&self) -> Result<()> {
        match kortex_platform::native::fs::remove_file(self.credentials_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(PkgError::new(
                Kind::CredentialStore,
                "jeton protégé, mais suppression du fichier historique impossible",
            )),
        }
    }
    pub fn load_credentials(&self) -> Result<Option<Credentials>> {
        let account = self.credential_account()?;
        if let Some(bytes) = self
            .secrets
            .read(CREDENTIAL_SERVICE, &account)
            .map_err(store_error)?
        {
            let c = parse_credentials(&bytes)?;
            // A prior interrupted migration must not leave an unnoticed plaintext copy.
            self.remove_legacy_credentials()?;
            return Ok(Some(c));
        }
        let bytes = match kortex_platform::native::fs::read_limited(
            self.credentials_path(),
            MAX_CREDENTIAL_BYTES,
        ) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(credential_error()),
        };
        let c = parse_credentials(&bytes)?;
        self.save_credentials(&c)?;
        Ok(Some(c))
    }
    pub fn save_credentials(&self, c: &Credentials) -> Result<()> {
        let bytes = serde_json::to_vec(c).map_err(|_| credential_error())?;
        parse_credentials(&bytes)?;
        let account = self.credential_account()?;
        let previous = self
            .secrets
            .read(CREDENTIAL_SERVICE, &account)
            .map_err(store_error)?;
        let result = (|| {
            self.secrets
                .write(CREDENTIAL_SERVICE, &account, &bytes)
                .map_err(store_error)?;
            let saved = self
                .secrets
                .read(CREDENTIAL_SERVICE, &account)
                .map_err(store_error)?;
            if saved.as_deref() != Some(bytes.as_slice()) {
                return Err(credential_error());
            }
            self.remove_legacy_credentials()
        })();
        if let Err(error) = result {
            let restored = match previous {
                Some(previous) => self.secrets.write(CREDENTIAL_SERVICE, &account, &previous),
                None => self.secrets.delete(CREDENTIAL_SERVICE, &account),
            };
            if restored.is_err() {
                return Err(PkgError::new(
                    Kind::CredentialStore,
                    "échec du trousseau et de sa restauration : reconnecte ce compte après déverrouillage",
                ));
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn clear_credentials(&self) -> Result<()> {
        let account = self.credential_account()?;
        // Remove a legacy copy first so a partial logout cannot resurrect it on next load.
        self.remove_legacy_credentials()?;
        self.secrets
            .delete(CREDENTIAL_SERVICE, &account)
            .map_err(store_error)
    }
}

struct Reply {
    status: u16,
    body: serde_json::Value,
}

impl Reply {
    fn error_code(&self) -> String {
        self.body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("error")
            .to_owned()
    }
}

fn call(
    api: &str,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&serde_json::Value>,
) -> Result<Reply> {
    if !(api.starts_with("https://") || is_local_host(api)) {
        return Err(PkgError::new(
            Kind::Unsafe,
            format!("API non chiffrée refusée : {api}"),
        ));
    }
    let url = format!("{}{path}", api.trim_end_matches('/'));
    let bearer = token.map(|t| format!("Bearer {t}"));
    let mut headers = vec![
        ("X-Kortex-Request", "1"),
        ("Content-Type", "application/json"),
    ];
    if let Some(value) = bearer.as_deref() {
        headers.push(("Authorization", value));
    }
    let payload = body.map_or_else(|| "{}".to_owned(), ToString::to_string);
    let response = kortex_platform::native::http(kortex_platform::native::HttpRequest {
        method,
        url: &url,
        headers: &headers,
        body: Some(payload.as_bytes()),
        timeout: Duration::from_secs(30),
        max_bytes: 1 << 20,
    })
    .map_err(|e| PkgError::new(Kind::Network, format!("serveur injoignable : {e}")))?;
    let status = response.status;
    let bytes = response.body;
    Ok(Reply {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    })
}

fn api_error(r: &Reply) -> PkgError {
    let code = r.error_code();
    let kind = match r.status {
        401 | 403 => Kind::BadSignature,
        404 => Kind::NotFound,
        429 => Kind::Network,
        _ => Kind::Parse,
    };
    PkgError::new(
        kind,
        match code.as_str() {
            "invalid_credentials" => "e-mail ou mot de passe incorrect".to_owned(),
            "rate_limited" => "trop de tentatives : réessaie dans quelques minutes".to_owned(),
            "too_many_devices" => {
                "trop d'appareils connectés : révoque-en un depuis ton compte".to_owned()
            }
            "unauthenticated" => "session expirée : relance `kortex login`".to_owned(),
            other => format!("le serveur a répondu {} ({other})", r.status),
        },
    )
}

/// Logs this machine in: derives the key locally, trades it for a revocable device token and stores it.
pub fn login_device(
    home: &Home,
    api: &str,
    email: &str,
    password: &str,
    device_name: &str,
    platform: &str,
    iterations: u32,
) -> Result<Credentials> {
    let body = serde_json::json!({
        "email": email.trim(),
        "auth_key": derive_auth_key(email, password, iterations),
        "kdf": KDF_VERSION,
        "name": device_name,
        "platform": platform,
    });
    let r = call(api, "POST", "/v1/auth/device", None, Some(&body))?;
    if r.status != 201 {
        return Err(api_error(&r));
    }
    let (Some(token), Some(device_id)) = (r.body["token"].as_str(), r.body["device_id"].as_str())
    else {
        return Err(PkgError::new(Kind::Parse, "réponse du serveur incomplète"));
    };
    let c = Credentials {
        api: api.to_owned(),
        email: email.trim().to_lowercase(),
        device_id: device_id.to_owned(),
        token: token.to_owned(),
    };
    if let Err(error) = home.save_credentials(&c) {
        let _ = call(&c.api, "DELETE", "/v1/devices/self", Some(&c.token), None);
        return Err(error);
    }
    Ok(c)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoutOutcome {
    Missing,
    LocalOnly,
    Revoked,
}

/// Clear local credentials even when the remote session cannot be revoked.
pub fn logout(home: &Home) -> Result<LogoutOutcome> {
    let c = match home.load_credentials() {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(LogoutOutcome::Missing),
        Err(error) if error.kind() == Kind::CredentialStore => {
            // Explicit logout can discard corrupt local data; backend deletion errors remain fatal.
            home.clear_credentials()?;
            return Ok(LogoutOutcome::LocalOnly);
        }
        Err(error) => return Err(error),
    };
    let remote = call(&c.api, "DELETE", "/v1/devices/self", Some(&c.token), None)
        .is_ok_and(|r| r.status == 200);
    home.clear_credentials()?;
    Ok(if remote {
        LogoutOutcome::Revoked
    } else {
        LogoutOutcome::LocalOnly
    })
}

/// `Some(email)` if the stored token is still valid, `None` if it was revoked or expired.
pub fn whoami(c: &Credentials) -> Result<Option<String>> {
    let r = call(&c.api, "GET", "/v1/me", Some(&c.token), None)?;
    Ok((r.status == 200).then(|| r.body["email"].as_str().unwrap_or(&c.email).to_owned()))
}

/// Whether installs must be confirmed with the password (account setting, on by default).
pub fn confirm_required(c: &Credentials) -> Result<bool> {
    let r = call(&c.api, "GET", "/v1/settings", Some(&c.token), None)?;
    if r.status != 200 {
        return Err(api_error(&r));
    }
    Ok(r.body["data"]["confirm_install"].as_bool().unwrap_or(true))
}

/// Re-checks the password with the server. `Ok(false)` = wrong password.
pub fn confirm_password(c: &Credentials, password: &str, iterations: u32) -> Result<bool> {
    let body = serde_json::json!({ "auth_key": derive_auth_key(&c.email, password, iterations), "kdf": KDF_VERSION });
    let r = call(
        &c.api,
        "POST",
        "/v1/auth/confirm",
        Some(&c.token),
        Some(&body),
    )?;
    match r.status {
        200 => Ok(true),
        403 => Ok(false),
        _ => Err(api_error(&r)),
    }
}

pub fn record_install(c: &Credentials, tool: &str, version: &str, target: &str) -> Result<()> {
    let r = call(
        &c.api,
        "POST",
        "/v1/installs",
        Some(&c.token),
        Some(&serde_json::json!({ "tool": tool, "version": version, "target": target })),
    )?;
    if r.status == 200 {
        Ok(())
    } else {
        Err(api_error(&r))
    }
}

pub fn forget_install(c: &Credentials, tool: &str) -> Result<()> {
    let r = call(
        &c.api,
        "DELETE",
        &format!("/v1/installs/{tool}"),
        Some(&c.token),
        None,
    )?;
    if r.status == 200 {
        Ok(())
    } else {
        Err(api_error(&r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vectors produced by services/shared/src/auth.ts (the website's implementation).
    #[test]
    fn matches_the_javascript_derivation_byte_for_byte() {
        assert_eq!(
            derive_auth_key("a@example.com", "correct horse battery", 1000),
            "adYcqHRz2aADufX9bEp0judmM4iW3Ap-q5qUu5Bp3ck"
        );
        assert_eq!(
            derive_auth_key("  Tiago@Example.COM ", "pässwörd ✓ long enough", 1000),
            "Ize5qupHQlT_JZKDihTRL711bn8YYToUiOt_FEVUGn0"
        );
        assert_eq!(
            derive_auth_key("a@example.com", "correct horse battery", 600_000),
            "RqrMB8NUef3m3Lw6lR70_wo4isDN2ULqrCy-7F-ZQSk"
        );
    }

    #[test]
    fn plain_http_to_remote_api_is_refused() {
        let e = call("http://api.evil.example", "GET", "/v1/me", None, None)
            .err()
            .expect("refused");
        assert_eq!(e.kind(), Kind::Unsafe);
    }

    fn test_home(name: &str) -> Home {
        let dir =
            std::env::temp_dir().join(format!("kortex-credential-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Home::at(dir.join("home"), &dir).with_secret_store(std::sync::Arc::new(
            kortex_platform::secrets::MemorySecretStore::default(),
        ))
    }
    fn credential() -> Credentials {
        Credentials {
            api: "https://api.kortex.site".into(),
            email: "a@example.com".into(),
            device_id: "d1".into(),
            token: "kdt_private_token".into(),
        }
    }
    #[test]
    fn protected_credentials_roundtrip_and_legacy_migration() {
        let home = test_home("migration");
        assert!(home.load_credentials().expect("read").is_none());
        let c = credential();
        std::fs::write(
            home.credentials_path(),
            serde_json::to_vec(&c).expect("json"),
        )
        .expect("legacy fixture");
        assert_eq!(home.load_credentials().expect("migration"), Some(c.clone()));
        assert!(!home.credentials_path().exists());
        assert_eq!(home.load_credentials().expect("protected read"), Some(c));
        home.clear_credentials().expect("clear");
        assert!(home.load_credentials().expect("read").is_none());
    }
    #[test]
    fn corrupt_legacy_and_debug_never_expose_or_accept_a_token() {
        let home = test_home("corrupt");
        std::fs::create_dir_all(home.root()).expect("dir");
        std::fs::write(home.credentials_path(), br#"{"token":"kdt_private_token"}"#)
            .expect("fixture");
        let error = home.load_credentials().expect_err("corrupt record");
        assert_eq!(error.kind(), Kind::CredentialStore);
        assert!(!format!("{error:?}").contains("kdt_private_token"));
        assert!(!format!("{:?}", credential()).contains("kdt_private_token"));
        assert!(
            home.credentials_path().exists(),
            "failed migration keeps recovery data"
        );
        assert_eq!(
            logout(&home).expect("explicit corrupt-data logout"),
            LogoutOutcome::LocalOnly
        );
        assert!(!home.credentials_path().exists());
        assert_eq!(
            logout(&home).expect("already logged out"),
            LogoutOutcome::Missing
        );
    }
    #[derive(Debug)]
    struct Locked;
    impl kortex_platform::secrets::SecretStore for Locked {
        fn read(
            &self,
            _: &str,
            _: &str,
        ) -> std::result::Result<Option<Vec<u8>>, kortex_platform::secrets::SecretError> {
            Err(kortex_platform::secrets::SecretError::LockedOrDenied)
        }
        fn write(
            &self,
            _: &str,
            _: &str,
            _: &[u8],
        ) -> std::result::Result<(), kortex_platform::secrets::SecretError> {
            Err(kortex_platform::secrets::SecretError::LockedOrDenied)
        }
        fn delete(
            &self,
            _: &str,
            _: &str,
        ) -> std::result::Result<(), kortex_platform::secrets::SecretError> {
            Err(kortex_platform::secrets::SecretError::LockedOrDenied)
        }
    }
    #[test]
    fn locked_store_never_uses_the_legacy_file_as_a_fallback() {
        let home = test_home("locked").with_secret_store(std::sync::Arc::new(Locked));
        std::fs::create_dir_all(home.root()).expect("dir");
        std::fs::write(
            home.credentials_path(),
            serde_json::to_vec(&credential()).expect("json"),
        )
        .expect("legacy");
        assert_eq!(
            home.load_credentials().expect_err("locked").kind(),
            Kind::CredentialStore
        );
        assert!(home.credentials_path().exists());
    }
    #[derive(Debug, Default)]
    struct FaultStore {
        inner: kortex_platform::secrets::MemorySecretStore,
        fail_write: std::sync::atomic::AtomicBool,
        fail_delete: std::sync::atomic::AtomicBool,
        corrupt_after_write: std::sync::atomic::AtomicBool,
        corrupt_next_read: std::sync::atomic::AtomicBool,
    }
    impl kortex_platform::secrets::SecretStore for FaultStore {
        fn read(
            &self,
            s: &str,
            a: &str,
        ) -> std::result::Result<Option<Vec<u8>>, kortex_platform::secrets::SecretError> {
            if self
                .corrupt_next_read
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                return Ok(Some(b"invalid-record".to_vec()));
            }
            self.inner.read(s, a)
        }
        fn write(
            &self,
            s: &str,
            a: &str,
            value: &[u8],
        ) -> std::result::Result<(), kortex_platform::secrets::SecretError> {
            use std::sync::atomic::Ordering::SeqCst;
            if self.fail_write.swap(false, SeqCst) {
                return Err(kortex_platform::secrets::SecretError::Backend);
            }
            self.inner.write(s, a, value)?;
            if self.corrupt_after_write.swap(false, SeqCst) {
                self.corrupt_next_read.store(true, SeqCst);
            }
            Ok(())
        }
        fn delete(
            &self,
            s: &str,
            a: &str,
        ) -> std::result::Result<(), kortex_platform::secrets::SecretError> {
            if self.fail_delete.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(kortex_platform::secrets::SecretError::LockedOrDenied);
            }
            self.inner.delete(s, a)
        }
    }
    #[test]
    fn failed_write_or_verification_restores_previous_credentials_and_failed_clear_is_visible() {
        use std::sync::atomic::Ordering::SeqCst;
        let store = std::sync::Arc::new(FaultStore::default());
        let home = test_home("rollback").with_secret_store(store.clone());
        let previous = credential();
        home.save_credentials(&previous)
            .expect("initial protected record");
        let mut next = previous.clone();
        next.token = "kdt_new_token".into();
        store.fail_write.store(true, SeqCst);
        assert!(home.save_credentials(&next).is_err());
        assert_eq!(
            home.load_credentials().expect("previous record"),
            Some(previous.clone())
        );
        store.corrupt_after_write.store(true, SeqCst);
        assert!(home.save_credentials(&next).is_err());
        assert_eq!(
            home.load_credentials().expect("previous record"),
            Some(previous.clone())
        );
        store.fail_delete.store(true, SeqCst);
        assert!(home.clear_credentials().is_err());
        assert_eq!(
            home.load_credentials().expect("still present"),
            Some(previous)
        );
    }
}
