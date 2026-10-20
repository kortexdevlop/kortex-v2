//! Runs a saved workflow without the canvas: for triggers (a schedule, "when opened", a local
//! webhook). Same engine and same runners as a click on Run, same history record; no live state.
use std::collections::{BTreeMap, BTreeSet};

use kortex_runtime::{CancelToken, Value};
use kortex_workflow::{Execution, NodeKind, Step, Workflow, order, validate};

use crate::catalogue::{doors, ports};
use crate::history::{Outcome as Shape, RunRecord, record};
use crate::ui::node_title;

/// What an automatic run came to.
pub struct Outcome {
    pub record: RunRecord,
    pub ok: bool,
    pub error: Option<String>,
}

fn failed(
    started: u64,
    label: &str,
    order: &[(String, String)],
    node: Option<&str>,
    error: String,
) -> Outcome {
    let now = kortex_platform::now_ms();
    let empty = BTreeSet::new();
    let ms = BTreeMap::new();
    let shape = Shape {
        workflow: label,
        at: started,
        total_ms: now.saturating_sub(started),
        order,
        ms: &ms,
        done: &empty,
        skipped: &empty,
        failed: Some((node.unwrap_or(""), error.clone())),
    };
    Outcome {
        record: record(&shape),
        ok: false,
        error: Some(error),
    }
}

/// Runs `workflow`. `inputs` maps an Input card's name to the value it receives (the body of a
/// webhook call arrives as `payload`).
pub async fn run(workflow: &Workflow, label: &str, inputs: &BTreeMap<String, String>) -> Outcome {
    let l = crate::ui::locale(&crate::ai::lang());
    let lang = crate::ai::lang();
    let started = kortex_platform::now_ms();
    let titles: Vec<(String, String)> = order(workflow)
        .unwrap_or_default()
        .into_iter()
        .map(|i| {
            (
                workflow.nodes[i].id.clone(),
                node_title(&workflow.nodes[i], l, &lang),
            )
        })
        .collect();
    let first_ai = workflow
        .nodes
        .iter()
        .find(|n| matches!(&n.kind, NodeKind::Logic { op, .. } if op == "ai"));
    if let Some(node) = first_ai.filter(|_| !crate::ai::consented()) {
        return failed(
            started,
            label,
            &titles,
            Some(&node.id),
            "consent_required".into(),
        );
    }
    if let Some(issue) = validate(workflow, ports()).first() {
        return failed(
            started,
            label,
            &titles,
            issue.node.as_deref(),
            format!("invalid_workflow ({})", issue.code),
        );
    }
    let (doors_in, _) = doors(workflow);
    let provided = doors_in
        .iter()
        .filter_map(|(port, node)| {
            inputs
                .get(port)
                .map(|v| (node.clone(), Value::Text(v.clone())))
        })
        .collect();
    let token = CancelToken::default();
    let mut execution = match Execution::new(workflow, ports()) {
        Ok(e) => e.with_files(provided),
        Err(f) => return failed(started, label, &titles, f.node.as_deref(), f.code),
    };
    let mut spent: BTreeMap<String, u64> = BTreeMap::new();
    loop {
        match execution.next(&token) {
            Step::Run(job) => {
                let began = kortex_platform::now_ms();
                let result = crate::runner::run(&job).await;
                *spent.entry(job.node.clone()).or_insert(0) +=
                    kortex_platform::now_ms().saturating_sub(began);
                execution.finish(result);
            }
            Step::Done(report) => {
                let done: BTreeSet<String> = report.results.keys().cloned().collect();
                let skipped: BTreeSet<String> = report.skipped.iter().cloned().collect();
                let shape = Shape {
                    workflow: label,
                    at: started,
                    total_ms: kortex_platform::now_ms().saturating_sub(started),
                    order: &titles,
                    ms: &spent,
                    done: &done,
                    skipped: &skipped,
                    failed: None,
                };
                return Outcome {
                    record: record(&shape),
                    ok: true,
                    error: None,
                };
            }
            Step::Failed(f) => {
                let done: BTreeSet<String> = f.report.results.keys().cloned().collect();
                let skipped: BTreeSet<String> = f.report.skipped.iter().cloned().collect();
                let error = format!(
                    "{}{}",
                    f.code,
                    f.detail
                        .as_deref()
                        .map(|d| format!(" ({d})"))
                        .unwrap_or_default()
                );
                let shape = Shape {
                    workflow: label,
                    at: started,
                    total_ms: kortex_platform::now_ms().saturating_sub(started),
                    order: &titles,
                    ms: &spent,
                    done: &done,
                    skipped: &skipped,
                    failed: Some((f.node.as_deref().unwrap_or(""), error.clone())),
                };
                return Outcome {
                    record: record(&shape),
                    ok: false,
                    error: Some(error),
                };
            }
        }
    }
}
