//! Desktop runner: executes installed tool binaries in their headless `--engine` mode
//! (JSON-RPC lines on stdio, MASTER_PROMPT §7.3). One short-lived process per node run, started
//! and killed through `kortex-platform`; nothing here touches the file system or the network.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use kortex_runtime::ValueMap;

use crate::engine::{RunError, Runner};

pub struct ProcessRunner {
    pub binaries: BTreeMap<String, PathBuf>,
    pub timeout: Duration,
}

fn failure(code: &str, detail: impl Into<String>) -> RunError {
    RunError {
        code: code.to_owned(),
        detail: Some(detail.into()),
    }
}

/// Reads the single JSON-RPC response of a `run` request.
///
/// # Errors
/// The tool's own error (its code and detail), or `worker_protocol` for an unreadable reply.
pub fn parse_response(text: &str) -> Result<ValueMap, RunError> {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| failure("worker_protocol", "empty reply"))?;
    let reply: serde_json::Value =
        serde_json::from_str(line).map_err(|e| failure("worker_protocol", e.to_string()))?;
    if let Some(error) = reply.get("error") {
        let data = error.get("data");
        let code = data
            .and_then(|d| d.get("code"))
            .or_else(|| error.get("message"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("worker_failed");
        let detail = data
            .and_then(|d| d.get("detail"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        return Err(RunError {
            code: code.to_owned(),
            detail,
        });
    }
    let outputs = reply
        .get("result")
        .and_then(|r| r.get("outputs"))
        .cloned()
        .ok_or_else(|| failure("worker_protocol", "no outputs"))?;
    serde_json::from_value(outputs).map_err(|e| failure("worker_protocol", e.to_string()))
}

impl Runner for ProcessRunner {
    fn run(&self, tool: &str, inputs: ValueMap, params: ValueMap) -> Result<ValueMap, RunError> {
        let path = self
            .binaries
            .get(tool)
            .ok_or_else(|| failure("tool_not_installed", tool))?;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "run",
            "params": {"inputs": inputs, "params": params},
        });
        let output = kortex_platform::native::process::exchange(
            path,
            &["--engine"],
            &format!("{request}\n"),
            self.timeout,
        )
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut {
                RunError {
                    code: "worker_timeout".to_owned(),
                    detail: None,
                }
            } else {
                failure("worker_failed", e.to_string())
            }
        })?;
        parse_response(&output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kortex_runtime::Value;

    #[test]
    fn parses_outputs_and_the_tools_own_error_codes() {
        let ok = r#"{"jsonrpc":"2.0","id":1,"result":{"outputs":{"text":{"Text":"hi"}}}}"#;
        assert_eq!(
            parse_response(ok).map(|m| m["text"].clone()),
            Ok(Value::Text("hi".into()))
        );
        let err = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"empty_input","data":{"code":"empty_input","detail":"x"}}}"#;
        assert_eq!(
            parse_response(err),
            Err(RunError {
                code: "empty_input".into(),
                detail: Some("x".into())
            })
        );
    }

    #[test]
    fn unreadable_replies_are_protocol_errors() {
        for bad in ["", "not json", r#"{"result":{}}"#] {
            assert_eq!(
                parse_response(bad).map_err(|e| e.code),
                Err("worker_protocol".to_owned()),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_missing_binary_is_reported_without_starting_anything() {
        let runner = ProcessRunner {
            binaries: BTreeMap::new(),
            timeout: Duration::from_secs(1),
        };
        assert_eq!(
            runner
                .run("text.cleaner", ValueMap::new(), ValueMap::new())
                .map_err(|e| e.code),
            Err("tool_not_installed".to_owned())
        );
    }
}
