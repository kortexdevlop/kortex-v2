//! Runs one job of a workflow locally: the tool's headless engine, never a server. Desktop starts
//! the installed binary in `--engine` mode on a background thread; the browser runs the
//! `engine.wasm` that Flow ships under `/engines/` in a Web Worker of its own origin.
use kortex_workflow::{Job, RunError};

/// Time a single tool may take.
pub const TIMEOUT_MS: u32 = 120_000;

#[cfg(target_arch = "wasm32")]
pub async fn run(job: &Job) -> Result<kortex_runtime::ValueMap, RunError> {
    let url = format!("/engines/{}/worker.js", job.tool);
    kortex_platform::worker::run(&url, job.inputs.clone(), job.params.clone(), TIMEOUT_MS)
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
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run(job: &Job) -> Result<kortex_runtime::ValueMap, RunError> {
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
pub fn cancel() {}
