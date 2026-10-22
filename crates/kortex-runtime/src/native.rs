//! Desktop execution: engines run on a dedicated thread (never on the UI thread) with
//! cancellation, a deadline and panic containment, plus the headless `--engine` JSON-RPC service.
//!
//! Cancellation is cooperative for the engine (it observes [`CancelToken`] through
//! `Cx::check_cancelled`) but immediate for the caller: the awaited future resolves with
//! `cancelled` at once. An engine that never checks keeps its thread until it finishes; its
//! result is dropped.
use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kortex_platform::native::task::{self, Slot};
use serde_json::{Value as Json, json};

use crate::{CancelToken, ToolError, ValueMap};

/// Stack of the thread an engine runs on (address space only): see `task::spawn_with_stack`.
const ENGINE_STACK: usize = 256 << 20;

/// Outcome of one engine run.
pub type RunOutcome = Result<ValueMap, ToolError>;

/// What a host provides: the blocking engine entry (`run_map` driven by `block_on`).
pub trait Engine:
    Fn(ValueMap, ValueMap, CancelToken) -> RunOutcome + Send + Sync + 'static
{
}
impl<F> Engine for F where
    F: Fn(ValueMap, ValueMap, CancelToken) -> RunOutcome + Send + Sync + 'static
{
}

type Current = Option<(CancelToken, Slot<RunOutcome>)>;

fn current() -> &'static Mutex<Current> {
    static CURRENT: Mutex<Current> = Mutex::new(None);
    &CURRENT
}

fn failure(code: &'static str, detail: &str) -> ToolError {
    ToolError::with_detail(code, detail)
}

fn start(
    inputs: ValueMap,
    params: ValueMap,
    timeout: Option<Duration>,
    engine: impl Engine,
) -> (CancelToken, Slot<RunOutcome>) {
    let cancel = CancelToken::default();
    let slot = Slot::new();
    let job = {
        let (cancel, slot) = (cancel.clone(), slot.clone());
        move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| engine(inputs, params, cancel)))
                .unwrap_or_else(|_| Err(failure("worker_failed", "engine panicked")));
            slot.complete(outcome);
        }
    };
    if task::spawn_with_stack("kortex-engine", ENGINE_STACK, job).is_err() {
        slot.complete(Err(failure("worker_failed", "thread unavailable")));
    }
    if let Some(timeout) = timeout {
        let (cancel, slot) = (cancel.clone(), slot.clone());
        // The watchdog ends as soon as the run completes, so it never outlives it by much.
        let _ = task::spawn("kortex-engine-deadline", move || {
            if !slot.wait_done(timeout) {
                cancel.cancel();
                slot.complete(Err(ToolError::new("worker_timeout")));
            }
        });
    }
    (cancel, slot)
}

/// Runs `engine` off the calling thread; the future resolves with its outcome, `cancelled`
/// after [`cancel_current`], or `worker_timeout` after `timeout`.
pub async fn run_isolated(
    inputs: ValueMap,
    params: ValueMap,
    timeout: Option<Duration>,
    engine: impl Engine,
) -> RunOutcome {
    let (cancel, slot) = start(inputs, params, timeout, engine);
    if let Ok(mut running) = current().lock() {
        *running = Some((cancel, slot.clone()));
    }
    slot.wait()
        .await
        .unwrap_or_else(|| Err(failure("worker_failed", "result lost")))
}

/// Blocking variant for headless hosts (CLI-style loops).
pub fn run_isolated_blocking(
    inputs: ValueMap,
    params: ValueMap,
    timeout: Option<Duration>,
    engine: impl Engine,
) -> RunOutcome {
    let (_, slot) = start(inputs, params, timeout, engine);
    slot.wait_blocking()
        .unwrap_or_else(|| Err(failure("worker_failed", "result lost")))
}

/// Cancels the run started last by [`run_isolated`]: the engine is told to stop and the awaiting
/// future resolves with `cancelled` immediately.
pub fn cancel_current() {
    let running = current().lock().ok().and_then(|mut guard| guard.take());
    if let Some((cancel, slot)) = running {
        cancel.cancel();
        slot.complete(Err(ToolError::new(ToolError::CANCELLED)));
    }
}

fn error_object(code: i64, message: &str, data: Json) -> Json {
    json!({"code": code, "message": message, "data": data})
}

fn respond(id: &Json, outcome: Result<Json, Json>) -> Json {
    match outcome {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    }
}

fn run_request(
    params: &Json,
    timeout: Option<Duration>,
    engine: &Arc<impl Engine>,
) -> Result<Json, Json> {
    let map = |key: &str| -> Result<ValueMap, Json> {
        match params.get(key) {
            None | Some(Json::Null) => Ok(ValueMap::new()),
            Some(value) => serde_json::from_value(value.clone()).map_err(|e| {
                error_object(
                    -32602,
                    "invalid params",
                    json!({"detail": format!("{key}: {e}")}),
                )
            }),
        }
    };
    let (inputs, params) = (map("inputs")?, map("params")?);
    let engine = Arc::clone(engine);
    let outcome = run_isolated_blocking(inputs, params, timeout, move |i, p, c| engine(i, p, c));
    match outcome {
        Ok(outputs) => Ok(json!({"outputs": outputs})),
        Err(error) => Err(error_object(
            -32000,
            error.code,
            json!({"code": error.code, "detail": error.detail}),
        )),
    }
}

/// JSON-RPC 2.0 over lines (one JSON object per line), the desktop `--engine` protocol.
///
/// Methods: `run` (`{"inputs": {...}, "params": {...}}` → `{"outputs": {...}}`), `contract`
/// (→ `{"contract": "<tool.toml>"}`), `shutdown`. Values use the runtime's serde form
/// (`{"Text": "x"}`, `{"Int": 3}`, …). Requests are served one at a time; a parent stops a
/// run by terminating the process.
///
/// # Errors
/// Only I/O errors on the streams; request errors are answered as JSON-RPC errors.
pub fn serve<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    contract: &str,
    timeout: Option<Duration>,
    engine: impl Engine,
) -> std::io::Result<()> {
    let engine = Arc::new(engine);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Json = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(e) => {
                let error = error_object(-32700, "parse error", json!({"detail": e.to_string()}));
                writeln!(writer, "{}", respond(&Json::Null, Err(error)))?;
                writer.flush()?;
                continue;
            }
        };
        let id = request.get("id").cloned().unwrap_or(Json::Null);
        let params = request.get("params").cloned().unwrap_or(Json::Null);
        let (reply, stop) = match request.get("method").and_then(Json::as_str) {
            Some("run") => (respond(&id, run_request(&params, timeout, &engine)), false),
            Some("contract") => (respond(&id, Ok(json!({"contract": contract}))), false),
            Some("shutdown") => (respond(&id, Ok(Json::Null)), true),
            Some(other) => (
                respond(
                    &id,
                    Err(error_object(
                        -32601,
                        "method not found",
                        json!({"method": other}),
                    )),
                ),
                false,
            ),
            None => (
                respond(
                    &id,
                    Err(error_object(-32600, "invalid request", Json::Null)),
                ),
                false,
            ),
        };
        writeln!(writer, "{reply}")?;
        writer.flush()?;
        if stop {
            break;
        }
    }
    Ok(())
}

/// [`serve`] on the process's standard streams; returns the process exit code (0 or 1).
pub fn serve_stdio(contract: &str, timeout: Option<Duration>, engine: impl Engine) -> u8 {
    let (reader, writer) = kortex_platform::native::stdio::streams();
    u8::from(serve(reader, writer, contract, timeout, engine).is_err())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Value, block_on};

    /// `cancel_current` acts on the process-wide current run: async tests must not overlap.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn echo(inputs: ValueMap, _: ValueMap, _: CancelToken) -> RunOutcome {
        Ok(inputs)
    }

    fn text(value: &str) -> ValueMap {
        ValueMap::from([("text".to_owned(), Value::Text(value.to_owned()))])
    }

    #[test]
    fn engine_runs_on_another_thread_and_returns_its_outputs() {
        let _serial = serial();
        let caller = std::thread::current().id();
        let outcome = block_on(run_isolated(
            text("a"),
            ValueMap::new(),
            None,
            move |i, _, _| {
                assert_ne!(std::thread::current().id(), caller);
                Ok(i)
            },
        ));
        assert_eq!(outcome, Ok(text("a")));
    }

    #[test]
    fn engine_errors_and_panics_become_typed_errors() {
        let _serial = serial();
        let failing = block_on(run_isolated(
            ValueMap::new(),
            ValueMap::new(),
            None,
            |_, _, _| Err(ToolError::new("empty_input")),
        ));
        assert_eq!(failing, Err(ToolError::new("empty_input")));
        let panicking = block_on(run_isolated(
            ValueMap::new(),
            ValueMap::new(),
            None,
            |_, _, _| panic!("boom"),
        ));
        assert_eq!(panicking.map_err(|e| e.code), Err("worker_failed"));
    }

    #[test]
    fn deadline_cancels_a_run_that_never_finishes() {
        let _serial = serial();
        let started = std::time::Instant::now();
        let outcome = block_on(run_isolated(
            ValueMap::new(),
            ValueMap::new(),
            Some(Duration::from_millis(50)),
            |_, _, cancel| {
                while !cancel.is_cancelled() {
                    std::thread::yield_now();
                }
                Err(ToolError::new(ToolError::CANCELLED))
            },
        ));
        assert_eq!(outcome.map_err(|e| e.code), Err("worker_timeout"));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn cancel_current_resolves_immediately_and_stops_the_engine() {
        let _serial = serial();
        let observed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let seen = Arc::clone(&observed);
        let canceller = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(50));
            cancel_current();
        });
        let outcome = block_on(run_isolated(
            ValueMap::new(),
            ValueMap::new(),
            None,
            move |_, _, cancel| {
                while !cancel.is_cancelled() {
                    std::thread::yield_now();
                }
                seen.store(true, std::sync::atomic::Ordering::SeqCst);
                Err(ToolError::new(ToolError::CANCELLED))
            },
        ));
        canceller.join().unwrap_or_default();
        assert_eq!(outcome.map_err(|e| e.code), Err(ToolError::CANCELLED));
        for _ in 0..200 {
            if observed.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        unreachable!("the engine never observed the cancellation");
    }

    fn exchange(requests: &str, timeout: Option<Duration>) -> Vec<Json> {
        let mut out = Vec::new();
        serve(requests.as_bytes(), &mut out, "schema = 1", timeout, echo)
            .unwrap_or_else(|e| unreachable!("{e}"));
        String::from_utf8_lossy(&out)
            .lines()
            .map(|l| serde_json::from_str(l).unwrap_or_default())
            .collect()
    }

    #[test]
    fn serve_runs_requests_and_echoes_ids() {
        let replies = exchange(
            concat!(
                r#"{"jsonrpc":"2.0","id":1,"method":"run","params":{"inputs":{"text":{"Text":"hi"}}}}"#,
                "\n",
                r#"{"jsonrpc":"2.0","id":"b","method":"contract"}"#,
                "\n",
                r#"{"jsonrpc":"2.0","id":3,"method":"shutdown"}"#,
                "\n",
                r#"{"jsonrpc":"2.0","id":4,"method":"contract"}"#,
                "\n",
            ),
            None,
        );
        assert_eq!(replies.len(), 3, "nothing is served after shutdown");
        assert_eq!(replies[0]["id"], 1);
        assert_eq!(replies[0]["result"]["outputs"]["text"]["Text"], "hi");
        assert_eq!(replies[1]["id"], "b");
        assert_eq!(replies[1]["result"]["contract"], "schema = 1");
        assert_eq!(replies[2]["result"], Json::Null);
    }

    #[test]
    fn serve_reports_protocol_errors_without_stopping() {
        let replies = exchange(
            concat!(
                "not json\n",
                r#"{"id":1,"method":"nope"}"#,
                "\n",
                r#"{"id":2}"#,
                "\n",
                r#"{"id":3,"method":"run","params":{"inputs":{"text":5}}}"#,
                "\n",
                r#"{"id":4,"method":"run","params":{"inputs":{}}}"#,
                "\n",
            ),
            None,
        );
        let codes: Vec<_> = replies
            .iter()
            .map(|r| r["error"]["code"].as_i64())
            .collect();
        assert_eq!(
            codes,
            [Some(-32700), Some(-32601), Some(-32600), Some(-32602), None]
        );
        assert!(replies[4]["result"]["outputs"].is_object());
    }

    #[test]
    fn serve_maps_engine_errors_to_their_code() {
        let mut out = Vec::new();
        let request = r#"{"id":1,"method":"run","params":{}}"#.to_owned() + "\n";
        serve(request.as_bytes(), &mut out, "", None, |_, _, _| {
            Err(ToolError::with_detail("empty_input", "no text"))
        })
        .unwrap_or_else(|e| unreachable!("{e}"));
        let reply: Json = serde_json::from_slice(&out).unwrap_or_default();
        assert_eq!(reply["error"]["message"], "empty_input");
        assert_eq!(reply["error"]["data"]["detail"], "no text");
    }
}
