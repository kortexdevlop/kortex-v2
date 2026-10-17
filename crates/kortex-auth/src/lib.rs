//! Shared typed account client. Browser cookies remain HttpOnly on the API origin.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::sync::OnceLock;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    SignedOut,
    SessionChanged,
    Network,
    InvalidResponse,
    RateLimited,
    Conflict,
    Unsupported,
    TooLarge,
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub email: String,
    pub created_at: u64,
}
pub fn supported() -> bool {
    kortex_platform::document::cookie_session_supported()
}
pub fn api() -> &'static str {
    static API: OnceLock<String> = OnceLock::new();
    API.get_or_init(|| {
        toml::from_str::<toml::Table>(include_str!("../../../brand/domains.toml"))
            .ok()
            .and_then(|v| {
                v.get("domains")?
                    .get("api")?
                    .as_str()
                    .map(|s| format!("https://{s}"))
            })
            .unwrap_or_default()
    })
}
pub async fn request(
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<kortex_platform::document::AccountResponse> {
    request_as(None, method, path, body).await
}
pub async fn request_as(
    user: Option<&str>,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<kortex_platform::document::AccountResponse> {
    if !supported() {
        return Err(Error::Unsupported);
    }
    if !path.starts_with("/v1/") || path.contains(['?', '#', '\\']) {
        return Err(Error::InvalidResponse);
    }
    let response =
        kortex_platform::document::account_request(&format!("{}{path}", api()), method, body, user)
            .await
            .map_err(|_| Error::Network)?;
    match response.status {
        401 | 403 => Err(Error::SignedOut),
        429 => Err(Error::RateLimited),
        409 if serde_json::from_str::<serde_json::Value>(&response.body)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .as_deref()
            == Some("session_changed") =>
        {
            Err(Error::SessionChanged)
        }
        _ => Ok(response),
    }
}
pub fn decode<T: DeserializeOwned>(body: &str) -> Result<T> {
    serde_json::from_str(body).map_err(|_| Error::InvalidResponse)
}
pub async fn session() -> Result<Option<Profile>> {
    let response = match request("GET", "/v1/me", None).await {
        Err(Error::SignedOut) => return Ok(None),
        other => other?,
    };
    if response.status != 200 {
        return Err(Error::Network);
    }
    let user: Profile = decode(&response.body)?;
    if user.id.is_empty() || user.id.len() > 128 || user.email.is_empty() || user.email.len() > 320
    {
        return Err(Error::InvalidResponse);
    }
    Ok(Some(user))
}
pub async fn verify_user(expected: &str) -> Result<()> {
    match session().await? {
        Some(user) if user.id == expected => Ok(()),
        Some(_) => Err(Error::SessionChanged),
        None => Err(Error::SignedOut),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_profiles_are_not_authenticated() {
        assert!(decode::<Profile>(r#"{"email":"a@example.test"}"#).is_err());
        assert!(decode::<Profile>("null").is_err());
        assert_eq!(api(), "https://api.kortex.site");
    }
}
