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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub api: String,
    pub email: String,
    pub device_id: String,
    pub token: String,
}

impl Home {
    #[must_use]
    pub fn credentials_path(&self) -> std::path::PathBuf {
        self.root().join("credentials.json")
    }

    pub fn load_credentials(&self) -> Option<Credentials> {
        serde_json::from_slice(&std::fs::read(self.credentials_path()).ok()?).ok()
    }

    /// Written with owner-only permissions; the token is as sensitive as a password.
    pub fn save_credentials(&self, c: &Credentials) -> Result<()> {
        std::fs::create_dir_all(self.root())?;
        let tmp = self
            .root()
            .join(format!(".credentials.{}.tmp", std::process::id()));
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(c).map_err(|e| PkgError::new(Kind::Parse, e.to_string()))?,
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, self.credentials_path())?;
        Ok(())
    }

    pub fn clear_credentials(&self) {
        let _ = std::fs::remove_file(self.credentials_path());
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
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .into();
    let url = format!("{}{path}", api.trim_end_matches('/'));
    let net =
        |e: ureq::Error| PkgError::new(Kind::Network, format!("serveur injoignable ({url}) : {e}"));
    let bearer = token.map(|t| format!("Bearer {t}"));
    let mut res = match method {
        "GET" | "DELETE" => {
            let mut r = if method == "GET" {
                agent.get(&url)
            } else {
                agent.delete(&url)
            }
            .header("X-Kortex-Request", "1");
            if let Some(b) = &bearer {
                r = r.header("Authorization", b);
            }
            r.call().map_err(net)?
        }
        _ => {
            let mut r = agent
                .post(&url)
                .header("X-Kortex-Request", "1")
                .content_type("application/json");
            if let Some(b) = &bearer {
                r = r.header("Authorization", b);
            }
            r.send(body.map_or_else(|| "{}".to_owned(), ToString::to_string))
                .map_err(net)?
        }
    };
    let status = res.status().as_u16();
    let bytes = res
        .body_mut()
        .with_config()
        .limit(1 << 20)
        .read_to_vec()
        .map_err(|e| PkgError::new(Kind::Network, format!("réponse illisible : {e}")))?;
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
    home.save_credentials(&c)?;
    Ok(c)
}

/// Revokes the device token on the server (best effort: offline logout still clears the local copy).
pub fn logout(home: &Home) -> Result<bool> {
    let Some(c) = home.load_credentials() else {
        return Ok(false);
    };
    let remote = call(&c.api, "DELETE", "/v1/devices/self", Some(&c.token), None)
        .is_ok_and(|r| r.status == 200);
    home.clear_credentials();
    Ok(remote)
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

    #[test]
    fn credentials_file_is_private_and_clearable() {
        let dir = std::env::temp_dir().join(format!("kortex-pkg-cred-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let home = Home::at(dir.join(".kortex"), &dir);
        assert!(home.load_credentials().is_none());
        let c = Credentials {
            api: "https://api.kortex.site".into(),
            email: "a@example.com".into(),
            device_id: "d1".into(),
            token: "kdt_x".into(),
        };
        home.save_credentials(&c).expect("save");
        assert_eq!(home.load_credentials(), Some(c));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(home.credentials_path())
                    .expect("meta")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        home.clear_credentials();
        assert!(home.load_credentials().is_none());
    }
}
