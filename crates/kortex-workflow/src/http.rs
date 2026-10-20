//! The HTTP node: policy and request building, pure. The same rules run before a desktop request
//! and are mirrored by the web proxy (`services/api/src/routes/proxy.ts`), so a workflow imported
//! from anywhere can only reach public HTTPS hosts. Secrets are placeholders (`{secret:NAME}`)
//! resolved by the host at run time, never stored in the workflow.
use kortex_runtime::{Value, ValueMap};

use crate::RunError;

/// Pseudo tool name of the HTTP job: the host performs the request.
pub const HTTP_TOOL: &str = "flow.http";
pub const METHODS: [&str; 5] = ["GET", "POST", "PUT", "PATCH", "DELETE"];
/// Request headers a workflow may set (the web proxy accepts the same list).
pub const HEADERS: [&str; 6] = [
    "accept",
    "accept-language",
    "content-type",
    "authorization",
    "x-api-key",
    "notion-version",
];
const MAX_HEADERS: usize = 12;
const BLOCKED_SUFFIXES: [&str; 10] = [
    ".local",
    ".localhost",
    ".internal",
    ".intranet",
    ".lan",
    ".home",
    ".corp",
    ".test",
    ".invalid",
    ".kortex.site",
];

/// A validated request, ready for a host to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub fail_on_error: bool,
}

fn error(code: &str, detail: impl Into<String>) -> RunError {
    RunError {
        code: code.to_owned(),
        detail: Some(detail.into()).filter(|d| !d.is_empty()),
    }
}

fn public_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let host = host.trim_end_matches('.');
    let labels: Vec<&str> = host.split('.').collect();
    let numeric = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == 'x');
    let Some(tld) = labels.last() else {
        return false;
    };
    !(numeric
        || host.contains(':')
        || host.contains('[')
        || labels.len() < 2
        || host == "kortex.site"
        || labels.iter().any(|l| l.starts_with("0x"))
        || BLOCKED_SUFFIXES.iter().any(|s| host.ends_with(s))
        || !labels.iter().all(|l| {
            (1..=63).contains(&l.len())
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !l.starts_with('-')
                && !l.ends_with('-')
        })
        || !(tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_lowercase())
            || tld.starts_with("xn--")))
}

/// Only HTTPS, port 443, no credentials, and a public DNS name (no IP literal in any notation,
/// single-label or internal names, nor our own domains).
///
/// # Errors
/// A short code: `invalid_url`, `https_only`, `credentials_in_url`, `port_not_allowed`,
/// `host_not_allowed`.
pub fn check_url(url: &str) -> Result<(), &'static str> {
    if url.is_empty()
        || url.len() > 2000
        || url.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err("invalid_url");
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return Err("https_only");
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return Err("credentials_in_url");
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !authority.starts_with('[') => (h, Some(p)),
        _ => (authority, None),
    };
    if port.is_some_and(|p| p != "443" && !p.is_empty()) {
        return Err("port_not_allowed");
    }
    if public_host(host) {
        Ok(())
    } else {
        Err("host_not_allowed")
    }
}

/// `Name: value` lines. Names outside [`HEADERS`] are refused, values cannot carry line breaks.
///
/// # Errors
/// `bad_header` with the offending name, `too_many_headers`.
pub fn parse_headers(text: &str) -> Result<Vec<(String, String)>, RunError> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some((name, value)) = line.split_once(':') else {
            return Err(error("bad_header", line));
        };
        let name = name.trim().to_ascii_lowercase();
        if !HEADERS.contains(&name.as_str()) || value.trim().len() > 2000 {
            return Err(error("bad_header", name));
        }
        out.push((name, value.trim().to_owned()));
    }
    if out.len() > MAX_HEADERS {
        return Err(error("too_many_headers", MAX_HEADERS.to_string()));
    }
    Ok(out)
}

/// Replaces `{secret:NAME}` by the value the host holds.
///
/// # Errors
/// `missing_secret` naming the secret.
pub fn substitute(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> Result<String, RunError> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{secret:") {
        out.push_str(&rest[..start]);
        let after = &rest[start + "{secret:".len()..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return Ok(out);
        };
        let name = &after[..end];
        if name.is_empty()
            || name.len() > 40
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            out.push_str("{secret:");
            rest = after;
            continue;
        }
        out.push_str(&lookup(name).ok_or_else(|| error("missing_secret", name))?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn text(map: &ValueMap, key: &str) -> Option<String> {
    match map.get(key) {
        Some(Value::Text(t)) => Some(t.clone()),
        _ => None,
    }
}

/// Builds the request of a job. Secrets are resolved only in the parameters the user wrote (URL,
/// headers, body), never in text that arrives from another node.
///
/// # Errors
/// A [`RunError`] whose code the UI translates (`bad_method`, URL codes, `bad_header`,
/// `missing_secret`, `body_not_allowed`).
pub fn build_request(
    inputs: &ValueMap,
    params: &ValueMap,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<Request, RunError> {
    let method = text(params, "method").unwrap_or_else(|| "GET".into());
    let url = match text(inputs, "url").filter(|u| !u.trim().is_empty()) {
        Some(from_node) => from_node.trim().to_owned(),
        None => substitute(text(params, "url").unwrap_or_default().trim(), lookup)?,
    };
    let headers = substitute(&text(params, "headers").unwrap_or_default(), lookup)?;
    let body = match text(inputs, "body") {
        Some(from_node) => Some(from_node),
        None => {
            let own = text(params, "body").unwrap_or_default();
            (!own.is_empty())
                .then(|| substitute(&own, lookup))
                .transpose()?
        }
    };
    let fail_on_error = !matches!(params.get("fail_on_error"), Some(Value::Bool(false)));
    finalize(&method, &url, &headers, body, fail_on_error)
}

/// Checks an already-resolved request against the policy: method, URL, headers, body.
///
/// # Errors
/// The same codes as [`build_request`].
pub fn finalize(
    method: &str,
    url: &str,
    headers: &str,
    body: Option<String>,
    fail_on_error: bool,
) -> Result<Request, RunError> {
    if !METHODS.contains(&method) {
        return Err(error("bad_method", method));
    }
    check_url(url).map_err(|code| error(code, ""))?;
    let headers = parse_headers(headers)?;
    if body.is_some() && !matches!(method, "POST" | "PUT" | "PATCH") {
        return Err(error("body_not_allowed", method));
    }
    Ok(Request {
        method: method.to_owned(),
        url: url.to_owned(),
        headers,
        body,
        fail_on_error,
    })
}

/// The node's outputs for a response: the body as text and the status code.
///
/// # Errors
/// `http_status` (naming the code) when `fail_on_error` and the status is 400 or more.
pub fn outputs(status: u16, body: &[u8], fail_on_error: bool) -> Result<ValueMap, RunError> {
    if fail_on_error && status >= 400 {
        return Err(error("http_status", status.to_string()));
    }
    Ok(ValueMap::from([
        (
            "text".to_owned(),
            Value::Text(String::from_utf8_lossy(body).into_owned()),
        ),
        ("status".to_owned(), Value::Text(status.to_string())),
    ]))
}

/// Decodes standard base64 (the proxy's body encoding); `None` for anything malformed.
#[must_use]
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer = 0_u32;
    let mut bits = 0;
    for c in text.trim_end_matches('=').chars() {
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buffer >> bits) & 0xff).ok()?);
        }
    }
    Some(out)
}
