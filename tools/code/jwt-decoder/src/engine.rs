//! Decodes a JSON Web Token locally: header, payload, algorithm and the dates it carries.
//! The signature is NOT verified (that needs the secret or public key): this is for reading a token.
use kortex::runtime::{Cx, ToolError};
use serde_json::Value;

use crate::{Inputs, Outputs, Params};

/// Base64url without padding (also tolerates padding and the standard alphabet).
fn base64url(part: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(part.len() * 3 / 4);
    let (mut acc, mut bits) = (0_u32, 0_u32);
    for c in part.trim_end_matches('=').chars() {
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '-' | '+' => 62,
            '_' | '/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xFF).ok()?);
        }
    }
    // One leftover sextet cannot encode a byte: the input was truncated.
    (part.trim_end_matches('=').len() % 4 != 1).then_some(out)
}

fn json_part(part: &str, error: &'static str) -> Result<Value, ToolError> {
    let bytes = base64url(part).ok_or_else(|| ToolError::new(error))?;
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(value @ Value::Object(_)) => Ok(value),
        _ => Err(ToolError::new(error)),
    }
}

fn show(value: &Value, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
    .unwrap_or_default()
}

/// `iat` / `exp` (seconds since 1970) as an ISO UTC date; empty when absent or out of range.
fn date(payload: &Value, claim: &str) -> String {
    payload
        .get(claim)
        .and_then(Value::as_i64)
        .and_then(|seconds| seconds.checked_mul(1000))
        .and_then(kortex_date::format_utc)
        .unwrap_or_default()
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let token = input.token.trim();
    let token = token
        .strip_prefix("Bearer ")
        .or_else(|| token.strip_prefix("bearer "))
        .unwrap_or(token)
        .trim();
    if token.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let parts: Vec<&str> = token.split('.').collect();
    match parts.len() {
        3 | 2 => {}
        5 => return Err(ToolError::new("encrypted_token")),
        _ => return Err(ToolError::new("invalid_token")),
    }
    let header = json_part(parts[0], "invalid_header")?;
    let payload = json_part(parts[1], "invalid_payload")?;
    cx.progress(1.0);
    Ok(Outputs {
        algorithm: header
            .get("alg")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        issued_at: date(&payload, "iat"),
        expires_at: date(&payload, "exp"),
        header: show(&header, params.pretty),
        payload: show(&payload, params.pretty),
    })
}
