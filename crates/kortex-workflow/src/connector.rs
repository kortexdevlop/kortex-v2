//! Connector contracts (`apps/flow/connectors/<name>/connector.toml`): a connector is a set of
//! actions, each a templated HTTP request on top of the HTTP node's policy. An action shows up in
//! Flow as a tool `connector.<id>.<action>` with one optional text input per parameter. Pure: the
//! host resolves secrets and sends the request.
//!
//! Templates use `{param}`, `{param|json}` (a JSON string literal) and `{param|url}` (percent
//! encoded), and `{secret:NAME}` for a device secret. Secrets are resolved in the *template*
//! before any user value is inserted, so a value arriving from another node can never name one.
use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::http::{self, Request};
use crate::{ParamSpec, Port, RunError, ToolPorts};
use kortex_contract::PortType;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Connector {
    pub id: String,
    pub name: BTreeMap<String, String>,
    #[serde(default)]
    pub help: BTreeMap<String, String>,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Action {
    pub id: String,
    pub name: BTreeMap<String, String>,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Param {
    pub id: String,
    pub label: BTreeMap<String, String>,
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub multiline: bool,
}

#[derive(Deserialize)]
struct File {
    #[allow(dead_code)]
    schema: u32,
    connector: Head,
    #[serde(default)]
    actions: Vec<Action>,
}
#[derive(Deserialize)]
struct Head {
    id: String,
    name: BTreeMap<String, String>,
    #[serde(default)]
    help: BTreeMap<String, String>,
}

fn safe_id(id: &str) -> bool {
    (1..=40).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// A placeholder starting at the beginning of `text` (`{name}` or `{name|filter}`): its name,
/// filter and the length it spans. JSON braces and `{secret:…}` are not placeholders.
fn placeholder_at(text: &str) -> Option<(&str, Option<&str>, usize)> {
    let inner = text.strip_prefix('{')?;
    let end = inner.find('}')?;
    let body = &inner[..end];
    let (name, filter) = match body.split_once('|') {
        Some((n, f)) => (n, Some(f)),
        None => (body, None),
    };
    let word = |w: &str| {
        !w.is_empty()
            && w.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    };
    (word(name) && filter.is_none_or(word)).then_some((name, filter, end + 2))
}

/// Placeholders of a template: `(name, filter)` pairs.
fn placeholders(template: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        rest = &rest[start..];
        match placeholder_at(rest) {
            Some((name, filter, len)) => {
                out.push((name.to_owned(), filter.map(str::to_owned)));
                rest = &rest[len..];
            }
            None => rest = &rest[1..],
        }
    }
    out
}

/// Parses and validates a contract.
///
/// # Errors
/// A message naming the first problem (bad id, duplicate action or parameter, unknown method,
/// non-HTTPS URL, placeholder without a parameter, unknown filter, missing French/English text).
pub fn parse(text: &str) -> Result<Connector, String> {
    let file: File = toml::from_str(text).map_err(|e| e.to_string())?;
    let c = file.connector;
    if !safe_id(&c.id) {
        return Err(format!("connector id `{}`", c.id));
    }
    let both = |m: &BTreeMap<String, String>| m.contains_key("fr") && m.contains_key("en");
    if !both(&c.name) {
        return Err(format!("{}: name needs fr and en", c.id));
    }
    let mut seen = BTreeSet::new();
    for a in &file.actions {
        if !safe_id(&a.id) || !seen.insert(a.id.clone()) || !both(&a.name) {
            return Err(format!("{}.{}: bad or duplicate action", c.id, a.id));
        }
        if !http::METHODS.contains(&a.method.as_str()) {
            return Err(format!("{}.{}: method {}", c.id, a.id, a.method));
        }
        if !a.url.starts_with("https://")
            && !a.url.starts_with("{secret:")
            && !a.url.starts_with("{")
        {
            return Err(format!("{}.{}: the URL must be HTTPS", c.id, a.id));
        }
        let mut ids = BTreeSet::new();
        for p in &a.params {
            if !safe_id(&p.id) || !ids.insert(p.id.clone()) || !both(&p.label) {
                return Err(format!("{}.{}.{}: bad parameter", c.id, a.id, p.id));
            }
        }
        let templates = std::iter::once(a.url.as_str())
            .chain(a.headers.iter().map(String::as_str))
            .chain(a.body.as_deref());
        for template in templates {
            for (name, filter) in placeholders(template) {
                if !ids.contains(&name) {
                    return Err(format!(
                        "{}.{}: `{{{name}}}` is not a parameter",
                        c.id, a.id
                    ));
                }
                if filter.is_some_and(|f| !matches!(f.as_str(), "json" | "url" | "raw")) {
                    return Err(format!("{}.{}: unknown filter on `{name}`", c.id, a.id));
                }
            }
        }
    }
    Ok(Connector {
        id: c.id,
        name: c.name,
        help: c.help,
        actions: file.actions,
    })
}

impl Connector {
    /// The tool id of an action in the Flow catalogue.
    #[must_use]
    pub fn tool_id(&self, action: &str) -> String {
        format!("connector.{}.{action}", self.id)
    }

    #[must_use]
    pub fn action(&self, id: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.id == id)
    }
}

/// Splits `connector.<id>.<action>`.
#[must_use]
pub fn split_tool(tool: &str) -> Option<(&str, &str)> {
    tool.strip_prefix("connector.")?.split_once('.')
}

impl Action {
    /// Ports and params of the action as a Flow tool: every parameter can be typed in or fed by a
    /// text input of the same name; outputs are the response text and status.
    #[must_use]
    pub fn ports(&self) -> ToolPorts {
        let out = |id: &str| Port {
            id: id.to_owned(),
            ty: PortType::Text,
            required: false,
        };
        ToolPorts {
            inputs: self
                .params
                .iter()
                .map(|p| Port {
                    id: p.id.clone(),
                    ty: PortType::Text,
                    required: false,
                })
                .collect(),
            params: self
                .params
                .iter()
                .map(|p| ParamSpec {
                    id: p.id.clone(),
                    ty: PortType::Text,
                    default: p.default.clone(),
                    options: Vec::new(),
                })
                .collect(),
            outputs: vec![out("text"), out("status")],
        }
    }
}

fn json_literal(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn percent(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Inserts the parameter values into a template whose secrets are already resolved.
fn render(template: &str, values: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        match placeholder_at(rest) {
            Some((name, filter, len)) if values.contains_key(name) => {
                let value = &values[name];
                out.push_str(&match filter {
                    Some("json") => json_literal(value),
                    Some("url") => percent(value),
                    _ => value.clone(),
                });
                rest = &rest[len..];
            }
            _ => {
                out.push('{');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Builds the request of an action from the values of its parameters.
///
/// # Errors
/// `missing_param` (naming it) for an empty required parameter, `missing_secret`, or any policy
/// code of [`http::finalize`] (non-public host, forbidden header…).
pub fn expand(
    action: &Action,
    values: &BTreeMap<String, String>,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<Request, RunError> {
    for p in &action.params {
        if p.required && values.get(&p.id).is_none_or(|v| v.trim().is_empty()) {
            return Err(RunError {
                code: "missing_param".into(),
                detail: Some(p.id.clone()),
            });
        }
    }
    // Every declared parameter has a value (its default when none was given), so no placeholder
    // is ever left in the request.
    let mut values = values.clone();
    for p in &action.params {
        values
            .entry(p.id.clone())
            .or_insert_with(|| p.default.clone());
    }
    let values = &values;
    // Secrets first, from the contract's own text; user values go in afterwards.
    let url = render(&http::substitute(&action.url, lookup)?, values);
    let headers = render(
        &http::substitute(&action.headers.join("\n"), lookup)?,
        values,
    );
    let body = action
        .body
        .as_deref()
        .map(|b| http::substitute(b, lookup).map(|b| render(&b, values)))
        .transpose()?;
    http::finalize(&action.method, url.trim(), &headers, body, true)
}
