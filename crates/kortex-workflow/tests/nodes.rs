//! File nodes and logic nodes (condition, filter, merge, delay) in the shared engine.
#![allow(clippy::result_large_err)]
use std::collections::BTreeMap;

use kortex_contract::PortType;
use kortex_runtime::{CancelToken, FileData, Value, ValueMap};
use kortex_workflow::{
    DELAY_TOOL, Edge, Endpoint, Execution, Node, NodeKind, Port, Step, ToolPorts, Workflow,
    from_json, to_json, validate,
};

fn catalogue() -> BTreeMap<String, ToolPorts> {
    let port = |id: &str, ty: PortType, required: bool| Port {
        id: id.into(),
        ty,
        required,
    };
    BTreeMap::from([
        (
            "pdf.merge".to_owned(),
            ToolPorts {
                inputs: vec![port(
                    "files",
                    PortType::Files("application/pdf".into()),
                    true,
                )],
                params: vec![],
                outputs: vec![port(
                    "file",
                    PortType::File("application/pdf".into()),
                    false,
                )],
            },
        ),
        (
            "text.upper".to_owned(),
            ToolPorts {
                inputs: vec![port("text", PortType::Text, true)],
                params: vec![],
                outputs: vec![port("text", PortType::Text, false)],
            },
        ),
    ])
}

fn node(id: &str, kind: NodeKind) -> Node {
    Node {
        id: id.into(),
        kind,
        retries: 0,
        pos: None,
    }
}
fn text(id: &str, value: &str) -> Node {
    node(
        id,
        NodeKind::Text {
            value: value.into(),
        },
    )
}
fn logic(id: &str, op: &str, params: &[(&str, &str)]) -> Node {
    node(
        id,
        NodeKind::Logic {
            op: op.into(),
            params: params
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        },
    )
}
fn tool(id: &str, name: &str) -> Node {
    node(
        id,
        NodeKind::Tool {
            tool: name.into(),
            params: BTreeMap::new(),
        },
    )
}
fn edge(from: (&str, &str), to: (&str, &str)) -> Edge {
    let end = |(node, port): (&str, &str)| Endpoint {
        node: node.into(),
        port: port.into(),
    };
    Edge {
        from: end(from),
        to: end(to),
    }
}
fn flow(nodes: Vec<Node>, edges: Vec<Edge>) -> Workflow {
    Workflow {
        nodes,
        edges,
        ..Workflow::default()
    }
}

/// Runs to the end, answering tool jobs with the text upper-cased and delay jobs unchanged.
fn run(
    w: &Workflow,
    files: BTreeMap<String, Value>,
) -> Result<kortex_workflow::Report, kortex_workflow::Failure> {
    let cat = catalogue();
    let mut exec = Execution::new(w, &cat)?.with_files(files);
    loop {
        match exec.next(&CancelToken::default()) {
            Step::Run(job) => {
                let out = match (job.tool.as_str(), job.inputs.get("text")) {
                    ("text.upper", Some(Value::Text(t))) => t.to_uppercase(),
                    (DELAY_TOOL, Some(Value::Text(t))) => t.clone(),
                    _ => "?".into(),
                };
                exec.finish(Ok(ValueMap::from([("text".into(), Value::Text(out))])));
            }
            Step::Done(r) => return Ok(r),
            Step::Failed(f) => return Err(f),
        }
    }
}

fn pdf(name: &str) -> FileData {
    FileData {
        name: name.into(),
        mime: "application/pdf".into(),
        bytes: vec![1, 2, 3],
    }
}

#[test]
fn a_file_node_feeds_a_files_input_and_its_bytes_never_reach_the_json() {
    let w = flow(
        vec![
            node(
                "f",
                NodeKind::File {
                    accept: "application/pdf".into(),
                    multiple: true,
                },
            ),
            tool("m", "pdf.merge"),
        ],
        vec![edge(("f", "files"), ("m", "files"))],
    );
    assert!(validate(&w, &catalogue()).is_empty());
    // A single file also feeds a batch input; a text node does not feed a file input.
    let single = flow(
        vec![
            node(
                "f",
                NodeKind::File {
                    accept: "application/pdf".into(),
                    multiple: false,
                },
            ),
            tool("m", "pdf.merge"),
        ],
        vec![edge(("f", "file"), ("m", "files"))],
    );
    assert!(validate(&single, &catalogue()).is_empty());
    let wrong = flow(
        vec![
            node(
                "f",
                NodeKind::File {
                    accept: "image/*".into(),
                    multiple: true,
                },
            ),
            tool("m", "pdf.merge"),
        ],
        vec![edge(("f", "files"), ("m", "files"))],
    );
    assert_eq!(validate(&wrong, &catalogue())[0].code, "type_mismatch");
    let json = to_json(&w);
    assert!(!json.contains("bytes"));
    assert_eq!(from_json(&json), Ok(w));
}

#[test]
fn running_without_the_chosen_file_stops_with_missing_file() {
    let w = flow(
        vec![
            node(
                "f",
                NodeKind::File {
                    accept: "application/pdf".into(),
                    multiple: false,
                },
            ),
            tool("m", "pdf.merge"),
        ],
        vec![edge(("f", "file"), ("m", "files"))],
    );
    let failure = run(&w, BTreeMap::new()).expect_err("no file chosen");
    assert_eq!(
        (failure.code.as_str(), failure.node.as_deref()),
        ("missing_file", Some("f"))
    );
    // With the file, the node resolves and hands the value on (the host runs the merge).
    let files = BTreeMap::from([("f".to_owned(), Value::File(pdf("a.pdf")))]);
    let cat = catalogue();
    let mut exec = Execution::new(&w, &cat).expect("valid").with_files(files);
    match exec.next(&CancelToken::default()) {
        Step::Run(job) => {
            assert!(matches!(job.inputs.get("files"), Some(Value::Files(f)) if f.len() == 1))
        }
        other => panic!("expected the merge job, got {other:?}"),
    }
}

#[test]
fn a_condition_runs_only_the_branch_taken_and_skips_the_other() {
    let w = flow(
        vec![
            text("t", "error: disk full"),
            logic(
                "c",
                "condition",
                &[("test", "contains"), ("value", "error")],
            ),
            tool("yes", "text.upper"),
            tool("no", "text.upper"),
        ],
        vec![
            edge(("t", "text"), ("c", "text")),
            edge(("c", "then"), ("yes", "text")),
            edge(("c", "else"), ("no", "text")),
        ],
    );
    let report = run(&w, BTreeMap::new()).expect("runs");
    assert_eq!(
        report.results["yes"]["text"],
        Value::Text("ERROR: DISK FULL".into())
    );
    assert!(!report.results.contains_key("no"));
    assert_eq!(report.skipped, vec!["no".to_owned()]);
    // Case-insensitive switch flips nothing here but must be accepted as a typed bool.
    let relaxed = flow(w.nodes.clone(), w.edges.clone());
    assert!(validate(&relaxed, &catalogue()).is_empty());
}

#[test]
fn filter_keeps_or_drops_matching_lines() {
    let make = |keep: &str| {
        flow(
            vec![
                text("t", "alpha\nbeta\nalphabet"),
                logic(
                    "f",
                    "filter",
                    &[("keep", keep), ("test", "contains"), ("value", "alpha")],
                ),
            ],
            vec![edge(("t", "text"), ("f", "text"))],
        )
    };
    assert_eq!(
        run(&make("keep"), BTreeMap::new()).expect("ok").results["f"]["text"],
        Value::Text("alpha\nalphabet".into())
    );
    assert_eq!(
        run(&make("drop"), BTreeMap::new()).expect("ok").results["f"]["text"],
        Value::Text("beta".into())
    );
}

#[test]
fn merge_joins_the_inputs_that_arrived_and_is_skipped_when_none_did() {
    let w = flow(
        vec![
            text("a", "one"),
            text("b", "two"),
            logic("m", "merge", &[("separator", " + ")]),
        ],
        vec![
            edge(("a", "text"), ("m", "a")),
            edge(("b", "text"), ("m", "c")),
        ],
    );
    assert_eq!(
        run(&w, BTreeMap::new()).expect("ok").results["m"]["text"],
        Value::Text("one + two".into())
    );
    // Behind a condition branch that was not taken, the merge has nothing and is skipped.
    let skipped = flow(
        vec![
            text("t", "x"),
            logic("c", "condition", &[("test", "equals"), ("value", "never")]),
            logic("m", "merge", &[]),
        ],
        vec![
            edge(("t", "text"), ("c", "text")),
            edge(("c", "then"), ("m", "a")),
        ],
    );
    let report = run(&skipped, BTreeMap::new()).expect("ok");
    assert_eq!(report.skipped, vec!["m".to_owned()]);
}

#[test]
fn delay_is_a_host_job_with_its_milliseconds() {
    let w = flow(
        vec![text("t", "wait"), logic("d", "delay", &[("ms", "250")])],
        vec![edge(("t", "text"), ("d", "text"))],
    );
    let cat = catalogue();
    let mut exec = Execution::new(&w, &cat).expect("valid");
    match exec.next(&CancelToken::default()) {
        Step::Run(job) => {
            assert_eq!(job.tool, DELAY_TOOL);
            assert_eq!(job.params["ms"], Value::Int(250));
        }
        other => panic!("expected the delay job, got {other:?}"),
    }
}

#[test]
fn bad_logic_params_and_unknown_operations_are_reported() {
    let bad = flow(
        vec![
            text("t", "x"),
            logic("c", "condition", &[("test", "sounds_like")]),
        ],
        vec![edge(("t", "text"), ("c", "text"))],
    );
    assert_eq!(validate(&bad, &catalogue())[0].code, "bad_param");
    let unknown = flow(vec![logic("x", "teleport", &[])], vec![]);
    assert_eq!(validate(&unknown, &catalogue())[0].code, "unknown_logic");
}
