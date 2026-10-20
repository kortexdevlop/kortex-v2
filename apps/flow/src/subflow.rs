//! A saved workflow used as a card of another workflow. The card's inputs and outputs are the
//! workflow's Input and Output cards; running it is running that workflow with the values handed
//! in, then collecting what its Output cards produced. Nesting is bounded.
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};

use kortex_runtime::{CancelToken, Value, ValueMap};
use kortex_workflow::{Execution, Job, RunError, Step, validate};

use crate::catalogue::{doors, ports};
use crate::library::Library;

/// Most workflows inside workflows inside workflows.
pub const MAX_DEPTH: u8 = 4;
static DEPTH: AtomicU8 = AtomicU8::new(0);

fn failure(code: &str, detail: Option<String>) -> RunError {
    RunError {
        code: code.to_owned(),
        detail,
    }
}

/// Keeps the nesting count right even when a run returns early.
struct Level;
impl Level {
    fn enter() -> Option<Self> {
        (DEPTH.fetch_add(1, Ordering::SeqCst) < MAX_DEPTH)
            .then_some(Self)
            .or_else(|| {
                DEPTH.fetch_sub(1, Ordering::SeqCst);
                None
            })
    }
}
impl Drop for Level {
    fn drop(&mut self) {
        DEPTH.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Runs the saved workflow named by `job.tool` (`workflow.<id>`).
///
/// # Errors
/// `subflow_missing`, `subflow_too_deep`, `subflow_invalid` (naming the first problem), or the
/// error of the card that stopped the inner run.
pub fn run(job: Job) -> Pin<Box<dyn Future<Output = Result<ValueMap, RunError>>>> {
    Box::pin(async move {
        let id = job
            .tool
            .strip_prefix("workflow.")
            .unwrap_or_default()
            .to_owned();
        let (library, _) = kortex_shell::load::<Library>("flow", "library");
        let Some(saved) = library.get(&id) else {
            return Err(failure("subflow_missing", None));
        };
        let Some(_level) = Level::enter() else {
            return Err(failure("subflow_too_deep", None));
        };
        let workflow = saved.workflow.clone();
        if let Some(first) = validate(&workflow, ports()).first() {
            return Err(failure(
                "subflow_invalid",
                Some(format!(
                    "{} {}",
                    first.node.as_deref().unwrap_or("-"),
                    first.code
                )),
            ));
        }
        let (inputs, outputs) = doors(&workflow);
        let provided = inputs
            .iter()
            .filter_map(|(port, node)| match job.inputs.get(port) {
                Some(Value::Text(t)) => Some((node.clone(), Value::Text(t.clone()))),
                _ => None,
            })
            .collect();
        let token = CancelToken::default();
        let mut execution = Execution::new(&workflow, ports())
            .map_err(|f| failure(&f.code, f.detail))?
            .with_files(provided);
        loop {
            match execution.next(&token) {
                Step::Run(inner) => {
                    let result = crate::runner::run(&inner).await;
                    execution.finish(result);
                }
                Step::Done(report) => {
                    let mut out = ValueMap::new();
                    for (port, node) in outputs {
                        if let Some(Value::Text(t)) =
                            report.results.get(&node).and_then(|r| r.get("text"))
                        {
                            out.insert(port, Value::Text(t.clone()));
                        }
                    }
                    return Ok(out);
                }
                Step::Failed(f) => {
                    return Err(failure(
                        &f.code,
                        Some(match f.detail {
                            Some(d) => format!("{}: {d}", f.node.as_deref().unwrap_or("-")),
                            None => f.node.unwrap_or_default(),
                        }),
                    ));
                }
            }
        }
    })
}
