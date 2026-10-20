//! Credits as the gateway reports them: a percentage and a status, never neuron counts.
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Low,
    Empty,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credits {
    pub percent: u8,
    pub status: Status,
    pub resets_in_seconds: Option<u64>,
}

impl Credits {
    #[must_use]
    pub fn from_json(value: &Value) -> Option<Self> {
        let percent = u8::try_from(value.get("percent")?.as_u64()?.min(100)).ok()?;
        let status = match value.get("status")?.as_str()? {
            "ok" => Status::Ok,
            "low" => Status::Low,
            "empty" => Status::Empty,
            "paused" => Status::Paused,
            _ => return None,
        };
        Some(Self {
            percent,
            status,
            resets_in_seconds: value.get("resets_in_seconds").and_then(Value::as_u64),
        })
    }

    /// Whether another request can be sent right now.
    #[must_use]
    pub fn can_ask(&self) -> bool {
        matches!(self.status, Status::Ok | Status::Low)
    }

    /// `HH:MM` (UTC) when something changes, for "AI paused, back at HH:MM" messages.
    #[must_use]
    pub fn back_at_utc(&self, now_unix_seconds: u64) -> Option<String> {
        let at = now_unix_seconds + self.resets_in_seconds?;
        Some(format!("{:02}:{:02}", (at / 3600) % 24, (at / 60) % 60))
    }
}
