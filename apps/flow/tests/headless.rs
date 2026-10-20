//! Automatic runs: the same engine as a click on Run, recorded in the history, without a canvas.
use std::collections::BTreeMap;

use kortex_flow::engine::{Edge, Endpoint, Node, NodeKind, Workflow};
use kortex_flow::headless::run;
use kortex_runtime::block_on;

fn logic(id: &str, op: &str, params: &[(&str, &str)]) -> Node {
    Node {
        id: id.into(),
        kind: NodeKind::Logic {
            op: op.into(),
            params: params
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        },
        retries: 0,
        pos: None,
    }
}

fn edge(a: (&str, &str), b: (&str, &str)) -> Edge {
    Edge {
        from: Endpoint {
            node: a.0.into(),
            port: a.1.into(),
        },
        to: Endpoint {
            node: b.0.into(),
            port: b.1.into(),
        },
    }
}

fn echo() -> Workflow {
    Workflow {
        name: "Echo".into(),
        nodes: vec![
            logic("in", "input", &[("name", "payload"), ("default", "none")]),
            logic("t", "template", &[("template", "got {a}")]),
            logic("out", "output", &[("name", "result")]),
        ],
        edges: vec![
            edge(("in", "text"), ("t", "a")),
            edge(("t", "text"), ("out", "text")),
        ],
        ..Workflow::default()
    }
}

#[test]
fn a_triggered_run_is_recorded_and_receives_the_webhook_body() {
    let done = block_on(run(
        &echo(),
        "⏱ Echo",
        &BTreeMap::from([("payload".to_owned(), "hello".to_owned())]),
    ));
    assert!(done.ok && done.error.is_none());
    assert!(done.record.ok);
    assert_eq!(done.record.workflow, "⏱ Echo");
    assert_eq!(done.record.nodes.len(), 3);
    // Without a body the Input card keeps its default.
    assert!(block_on(run(&echo(), "⏱ Echo", &BTreeMap::new())).ok);
}

#[test]
fn an_ai_card_without_consent_never_runs_by_itself() {
    let mut w = echo();
    w.nodes.push(logic("ai", "ai", &[("prompt", "x")]));
    let failed = block_on(run(&w, "⏱ AI", &BTreeMap::new()));
    assert!(!failed.ok);
    assert_eq!(failed.error.as_deref(), Some("consent_required"));
    assert!(!failed.record.ok);
}

#[test]
fn an_invalid_workflow_fails_cleanly() {
    let mut w = echo();
    w.edges.clear();
    let failed = block_on(run(&w, "⏱ Broken", &BTreeMap::new()));
    assert!(!failed.ok);
}
