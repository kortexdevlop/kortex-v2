//! Runs one job of a workflow locally: the tool's headless engine, never a server. Desktop starts
//! the installed binary in `--engine` mode on a background thread; the browser runs the
//! `engine.wasm` that Flow ships under `/engines/` in a Web Worker of its own origin.
use kortex_runtime::{Value, ValueMap};
use kortex_workflow::http::{HTTP_TOOL, Request, build_request, check_url, outputs};
use kortex_workflow::{AI_TOOL, DELAY_TOOL, Job, MAX_DELAY_MS, RunError};

/// Runs one job: an Agent node asks the model (which may run local tools), anything else goes
/// straight to its host.
pub async fn run(job: &Job) -> Result<ValueMap, RunError> {
    if job.tool == AI_TOOL {
        return crate::ai::run(job).await;
    }
    run_tool(job).await
}

/// Time a single tool may take.
pub const TIMEOUT_MS: u32 = 120_000;

#[cfg(target_arch = "wasm32")]
pub async fn run_tool(job: &Job) -> Result<ValueMap, RunError> {
    if job.tool == DELAY_TOOL {
        return delay(job).await;
    }
    if job.tool == HTTP_TOOL {
        let request = build_request(&job.inputs, &job.params, &|name| crate::secrets::get(name))?;
        return send(request).await;
    }
    if job.tool.starts_with("connector.") {
        return send(connector_request(job)?).await;
    }
    if job.tool.starts_with("workflow.") {
        return crate::subflow::run(job.clone()).await;
    }
    let origin = format!("https://{}", kortex_shell::domain("run"));
    kortex_platform::worker::run_hosted(
        &origin,
        &job.tool,
        job.inputs.clone(),
        job.params.clone(),
        TIMEOUT_MS,
    )
    .await
    .map_err(|e| RunError {
        code: e.code,
        detail: e.detail,
    })
}

/// Stops the tool currently running (the browser terminates its Worker immediately).
#[cfg(target_arch = "wasm32")]
pub fn cancel() {
    kortex_platform::worker::cancel();
    kortex_ai_client::cancel();
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_tool(job: &Job) -> Result<ValueMap, RunError> {
    if job.tool == DELAY_TOOL {
        return delay(job).await;
    }
    if job.tool == HTTP_TOOL {
        let request = build_request(&job.inputs, &job.params, &|name| crate::secrets::get(name))?;
        return send(request).await;
    }
    if job.tool.starts_with("connector.") {
        return send(connector_request(job)?).await;
    }
    if job.tool.starts_with("workflow.") {
        return crate::subflow::run(job.clone()).await;
    }
    use kortex_platform::native::task::{Slot, spawn};
    use kortex_workflow::Runner;
    use kortex_workflow::process::ProcessRunner;

    let Some(path) = kortex_shell::installed_binary(&job.tool) else {
        return Err(RunError {
            code: "tool_not_installed".into(),
            detail: Some(job.tool.clone()),
        });
    };
    let slot = Slot::new();
    let remote = slot.clone();
    let job = job.clone();
    let spawned = spawn("kortex-flow-run", move || {
        let runner = ProcessRunner {
            binaries: std::collections::BTreeMap::from([(job.tool.clone(), path)]),
            timeout: std::time::Duration::from_millis(u64::from(TIMEOUT_MS)),
        };
        remote.complete(runner.run(&job.tool, job.inputs, job.params));
    });
    if spawned.is_err() {
        return Err(RunError {
            code: "worker_failed".into(),
            detail: Some("thread unavailable".into()),
        });
    }
    slot.wait().await.unwrap_or_else(|| {
        Err(RunError {
            code: "worker_failed".into(),
            detail: None,
        })
    })
}

/// Desktop tools are separate processes with their own time limit; there is no in-flight handle.
#[cfg(not(target_arch = "wasm32"))]
pub fn cancel() {
    kortex_ai_client::cancel();
}

/// The delay node: waits (the page's own timer, on web and desktop), then passes the text on.
async fn delay(job: &Job) -> Result<ValueMap, RunError> {
    let ms = match job.params.get("ms") {
        Some(Value::Int(ms)) => (*ms).clamp(0, MAX_DELAY_MS),
        _ => 0,
    };
    kortex_platform::document::sleep(u32::try_from(ms).unwrap_or(0)).await;
    Ok(ValueMap::from([(
        "text".to_owned(),
        job.inputs
            .get("text")
            .cloned()
            .unwrap_or_else(|| Value::Text(String::new())),
    )]))
}

fn failure(code: &str, detail: Option<String>) -> RunError {
    RunError {
        code: code.to_owned(),
        detail,
    }
}

/// The HTTP node in the browser: the request goes through the account API's proxy (CORS makes a
/// direct call impossible for most APIs), which needs a signed-in user and applies the same URL
/// policy as [`check_url`].
#[cfg(target_arch = "wasm32")]
async fn send(request: Request) -> Result<ValueMap, RunError> {
    let headers: serde_json::Map<String, serde_json::Value> = request
        .headers
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    let mut payload = serde_json::json!({
        "url": request.url,
        "method": request.method,
        "headers": headers,
    });
    if let (Some(body), Some(map)) = (&request.body, payload.as_object_mut()) {
        map.insert("body".into(), serde_json::Value::String(body.clone()));
    }
    let api = match kortex_platform::document::dev_api_override().await {
        Some(dev) => dev,
        None => format!("https://{}", kortex_shell::domain("api")),
    };
    let reply = kortex_platform::document::proxy_request(&api, &payload.to_string())
        .await
        .map_err(|e| failure(&e.0, None))?;
    let envelope: serde_json::Value = serde_json::from_str(&reply.body).unwrap_or_default();
    if reply.status != 200 {
        let code = envelope
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("proxy_failed");
        return Err(failure(code, None));
    }
    let status = envelope
        .get("status")
        .and_then(serde_json::Value::as_u64)
        .and_then(|s| u16::try_from(s).ok())
        .ok_or_else(|| failure("proxy_failed", None))?;
    let body = envelope
        .get("body_base64")
        .and_then(serde_json::Value::as_str)
        .and_then(kortex_workflow::http::decode_base64)
        .ok_or_else(|| failure("proxy_failed", None))?;
    outputs(status, &body, request.fail_on_error)
}

// Headers can contain arbitrary secrets; refuse redirects to a different authority.
#[cfg(any(not(target_arch = "wasm32"), test))]
fn same_origin(left: &str, right: &str) -> bool {
    fn authority(url: &str) -> Option<&str> {
        url.strip_prefix("https://")
            .map(|rest| rest.split(['/', '?', '#']).next().unwrap_or(""))
    }
    matches!((authority(left), authority(right)), (Some(a), Some(b)) if a.eq_ignore_ascii_case(b))
}

/// The HTTP node on desktop: a native request on a background thread, redirects followed by hand
/// (GET only, at most three) so every hop is checked against the same policy.
#[cfg(not(target_arch = "wasm32"))]
async fn send(request: Request) -> Result<ValueMap, RunError> {
    use kortex_platform::native::task::{Slot, spawn};
    use kortex_platform::native::{HttpRequest, http_once};

    let slot = Slot::new();
    let remote = slot.clone();
    let spawned = spawn("kortex-flow-http", move || {
        let mut url = request.url.clone();
        let mut method = request.method.clone();
        let mut body = request.body.clone();
        for hop in 0..=3 {
            let headers: Vec<(&str, &str)> = request
                .headers
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let response = http_once(HttpRequest {
                method: &method,
                url: &url,
                headers: &headers,
                body: body.as_deref().map(str::as_bytes),
                timeout: std::time::Duration::from_secs(20),
                max_bytes: 1_000_000,
            });
            let result = match response {
                Err(_) => Err(failure("http_failed", None)),
                Ok(r) => {
                    let redirect = (300..400).contains(&r.status)
                        && matches!(method.as_str(), "GET")
                        && hop < 3;
                    match r.location.filter(|_| redirect) {
                        Some(next) => {
                            let absolute = if next.starts_with("https://") {
                                next
                            } else if next.starts_with('/') {
                                let origin = url
                                    .split_once("://")
                                    .map(|(_, rest)| rest.split('/').next().unwrap_or(""))
                                    .unwrap_or("");
                                format!("https://{origin}{next}")
                            } else {
                                String::new()
                            };
                            if check_url(&absolute).is_err() || !same_origin(&url, &absolute) {
                                Err(failure("redirect_not_allowed", None))
                            } else {
                                url = absolute;
                                method = "GET".into();
                                body = None;
                                continue;
                            }
                        }
                        None => outputs(r.status, &r.body, request.fail_on_error),
                    }
                }
            };
            remote.complete(result);
            return;
        }
        remote.complete(Err(failure("redirect_not_allowed", None)));
    });
    if spawned.is_err() {
        return Err(failure("worker_failed", Some("thread unavailable".into())));
    }
    slot.wait()
        .await
        .unwrap_or_else(|| Err(failure("worker_failed", None)))
}

/// A connector action as a request: each parameter takes the text of its input when one is wired,
/// otherwise the value typed in the node.
fn connector_request(job: &Job) -> Result<Request, RunError> {
    let Some((_, action)) = crate::connectors::find(&job.tool) else {
        return Err(failure("unknown_tool", Some(job.tool.clone())));
    };
    let text = |map: &ValueMap, key: &str| match map.get(key) {
        Some(Value::Text(t)) => Some(t.clone()),
        _ => None,
    };
    let values = action
        .params
        .iter()
        .map(|p| {
            let value = text(&job.inputs, &p.id)
                .filter(|v| !v.is_empty())
                .or_else(|| text(&job.params, &p.id))
                .unwrap_or_default();
            (p.id.clone(), value)
        })
        .collect();
    kortex_workflow::connector::expand(action, &values, &|name| crate::secrets::get(name))
}

#[cfg(test)]
mod redirect_tests {
    use super::same_origin;
    #[test]
    fn secrets_never_cross_authorities() {
        assert!(same_origin(
            "https://api.example/a",
            "https://API.example/b"
        ));
        assert!(!same_origin(
            "https://api.example/a",
            "https://other.example/b"
        ));
        assert!(!same_origin(
            "https://api.example/a",
            "https://api.example:444/b"
        ));
        assert!(!same_origin(
            "https://api.example/a",
            "http://api.example/b"
        ));
    }
}
