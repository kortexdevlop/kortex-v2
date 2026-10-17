//! Versioned, explicit metadata snapshots. No implicit upload, token storage or automatic overwrite.
pub use kortex_auth::{Error, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Hub,
    Desk,
}
impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Hub => "hub.layout.v1",
            Self::Desk => "desk.layout.v1",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub lang: String,
    pub theme: String,
    pub mode: String,
}
impl Preferences {
    pub fn valid(&self) -> bool {
        ["fr", "en"].contains(&self.lang.as_str())
            && ["", "light", "dark"].contains(&self.theme.as_str())
            && ["simple", "medium", "pro"].contains(&self.mode.as_str())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot<T> {
    pub schema: u8,
    pub kind: Kind,
    pub data: T,
    pub preferences: Preferences,
}
impl<T: Serialize> Snapshot<T> {
    pub fn encode(&self) -> Result<String> {
        if self.schema != 1 || !self.preferences.valid() {
            return Err(Error::InvalidResponse);
        }
        let json = serde_json::to_string(self).map_err(|_| Error::InvalidResponse)?;
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|_| Error::InvalidResponse)?;
        if value
            .get("data")
            .and_then(|v| v.get("schema"))
            .and_then(serde_json::Value::as_u64)
            != Some(1)
        {
            return Err(Error::InvalidResponse);
        }
        if json.len() > 65_536 {
            return Err(Error::TooLarge);
        }
        Ok(json)
    }
}
#[derive(Clone)]
pub struct Remote<T> {
    pub version: u64,
    pub snapshot: Snapshot<T>,
}
#[derive(Deserialize)]
struct Response<T> {
    version: u64,
    data: Snapshot<T>,
}
pub fn parse<T: DeserializeOwned>(body: &str, kind: Kind) -> Result<Remote<T>> {
    if body.len() > 131_072 {
        return Err(Error::TooLarge);
    }
    let raw: serde_json::Value = kortex_auth::decode(body)?;
    if raw
        .get("data")
        .and_then(|v| v.get("data"))
        .and_then(|v| v.get("schema"))
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return Err(Error::InvalidResponse);
    }
    let value: Response<T> = kortex_auth::decode(body)?;
    if value.version == 0
        || value.version > 9_007_199_254_740_990
        || value.data.schema != 1
        || value.data.kind != kind
        || !value.data.preferences.valid()
    {
        return Err(Error::InvalidResponse);
    }
    Ok(Remote {
        version: value.version,
        snapshot: value.data,
    })
}
pub async fn read<T: DeserializeOwned>(user: &str, kind: Kind) -> Result<Option<Remote<T>>> {
    kortex_auth::verify_user(user).await?;
    let response =
        kortex_auth::request_as(Some(user), "GET", &format!("/v1/sync/{}", kind.key()), None)
            .await?;
    match response.status {
        404 => Ok(None),
        200 => parse(&response.body, kind).map(Some),
        _ => Err(Error::Network),
    }
}
pub async fn write<T: Serialize>(user: &str, expected: u64, snapshot: &Snapshot<T>) -> Result<u64> {
    if expected > 9_007_199_254_740_990 {
        return Err(Error::InvalidResponse);
    }
    let data = snapshot.encode()?;
    kortex_auth::verify_user(user).await?;
    let body = format!("{{\"version\":{expected},\"data\":{data}}}");
    let response = kortex_auth::request_as(
        Some(user),
        "PUT",
        &format!("/v1/sync/{}", snapshot.kind.key()),
        Some(&body),
    )
    .await?;
    match response.status {
        200 | 201 => {
            let result: serde_json::Value = kortex_auth::decode(&response.body)?;
            let version = result
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .ok_or(Error::InvalidResponse)?;
            if version != expected + 1 {
                return Err(Error::InvalidResponse);
            }
            Ok(version)
        }
        409 => Err(Error::Conflict),
        413 => Err(Error::TooLarge),
        _ => Err(Error::Network),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Snapshot<serde_json::Value> {
        Snapshot {
            schema: 1,
            kind: Kind::Hub,
            data: serde_json::json!({"schema":1,"tools":["secure.hash-calculator"]}),
            preferences: Preferences {
                lang: "fr".into(),
                theme: "dark".into(),
                mode: "medium".into(),
            },
        }
    }
    #[test]
    fn validates_versions_kind_preferences_and_schema() {
        let s = sample();
        let data = s.encode().expect("snapshot");
        let body = format!("{{\"version\":1,\"data\":{data}}}");
        assert_eq!(
            parse::<serde_json::Value>(&body, Kind::Hub)
                .expect("valid")
                .version,
            1
        );
        assert!(parse::<serde_json::Value>(&body, Kind::Desk).is_err());
        assert!(
            parse::<serde_json::Value>(&body.replace("\"version\":1", "\"version\":0"), Kind::Hub)
                .is_err()
        );
        assert!(
            parse::<serde_json::Value>(
                &body.replace("\"mode\":\"medium\"", "\"mode\":\"invalid\""),
                Kind::Hub
            )
            .is_err()
        );
        assert!(
            parse::<serde_json::Value>(&body.replace("\"schema\":1", "\"schema\":2"), Kind::Hub)
                .is_err()
        );
    }
    #[test]
    fn rejects_oversized_or_unknown_envelopes() {
        let mut s = sample();
        s.data = serde_json::json!({"schema":1,"name":"x".repeat(65_536)});
        assert_eq!(s.encode().err(), Some(Error::TooLarge));
        let mut unsupported = sample();
        unsupported.data = serde_json::json!({"schema":2});
        assert_eq!(unsupported.encode().err(), Some(Error::InvalidResponse));
        let body = format!(
            "{{\"version\":1,\"data\":{}}}",
            sample()
                .encode()
                .expect("snapshot")
                .replace("\"schema\":1", "\"schema\":1,\"files\":[]")
        );
        assert!(parse::<serde_json::Value>(&body, Kind::Hub).is_err());
    }
}
