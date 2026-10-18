use std::cell::RefCell;
use std::collections::BTreeMap;

use kortex_contract::PortType;
use kortex_runtime::{CancelToken, Table, ToolError, Value, ValueMap};
use kortex_workflow::{
    Edge, Endpoint, Execution, FORMAT_VERSION, Node, NodeKind, ParamSpec, Port, RunError, Runner,
    Step, ToolPorts, Workflow, adapt, from_json, order, param_values, run, to_json, validate,
};

fn port(id: &str, ty: PortType, required: bool) -> Port {
    Port {
        id: id.into(),
        ty,
        required,
    }
}

fn catalogue() -> BTreeMap<String, ToolPorts> {
    BTreeMap::from([
        (
            "text.upper".to_owned(),
            ToolPorts {
                inputs: vec![port("text", PortType::Text, true)],
                params: vec![ParamSpec {
                    id: "shout".into(),
                    ty: PortType::Bool,
                    default: "false".into(),
                    options: vec![],
                }],
                outputs: vec![port("text", PortType::Text, false)],
            },
        ),
        (
            "text.repeat".to_owned(),
            ToolPorts {
                inputs: vec![port("text", PortType::Text, true)],
                params: vec![
                    ParamSpec {
                        id: "times".into(),
                        ty: PortType::Integer,
                        default: "2".into(),
                        options: vec![],
                    },
                    ParamSpec {
                        id: "mode".into(),
                        ty: PortType::Enum,
                        default: "plain".into(),
                        options: vec!["plain".into(), "dash".into()],
                    },
                ],
                outputs: vec![
                    port("text", PortType::Text, false),
                    port("count", PortType::Integer, false),
                ],
            },
        ),
        (
            "file.sink".to_owned(),
            ToolPorts {
                inputs: vec![port("file", PortType::File("text/plain".into()), true)],
                params: vec![],
                outputs: vec![port("size", PortType::Integer, false)],
            },
        ),
        (
            "table.make".to_owned(),
            ToolPorts {
                inputs: vec![],
                params: vec![],
                outputs: vec![port("table", PortType::Table, false)],
            },
        ),
    ])
}

fn text(id: &str, value: &str) -> Node {
    Node {
        id: id.into(),
        kind: NodeKind::Text {
            value: value.into(),
        },
        retries: 0,
    }
}

fn tool(id: &str, tool: &str) -> Node {
    Node {
        id: id.into(),
        kind: NodeKind::Tool {
            tool: tool.into(),
            params: BTreeMap::new(),
        },
        retries: 0,
    }
}

fn edge(from: (&str, &str), to: (&str, &str)) -> Edge {
    Edge {
        from: Endpoint {
            node: from.0.into(),
            port: from.1.into(),
        },
        to: Endpoint {
            node: to.0.into(),
            port: to.1.into(),
        },
    }
}

fn workflow(nodes: Vec<Node>, edges: Vec<Edge>) -> Workflow {
    Workflow {
        nodes,
        edges,
        ..Workflow::default()
    }
}

fn codes(workflow: &Workflow) -> Vec<&'static str> {
    validate(workflow, &catalogue())
        .into_iter()
        .map(|i| i.code)
        .collect()
}

/// Upper-cases text and records every call.
struct Fake {
    calls: RefCell<Vec<String>>,
    failures_left: RefCell<u8>,
}

impl Fake {
    fn new(failures: u8) -> Self {
        Self {
            calls: RefCell::default(),
            failures_left: RefCell::new(failures),
        }
    }
}

impl Runner for Fake {
    fn run(&self, tool: &str, inputs: ValueMap, params: ValueMap) -> Result<ValueMap, RunError> {
        self.calls.borrow_mut().push(tool.to_owned());
        if *self.failures_left.borrow() > 0 {
            *self.failures_left.borrow_mut() -= 1;
            return Err(ToolError::new("worker_failed").into());
        }
        let text = match inputs.get("text") {
            Some(Value::Text(t)) => t.clone(),
            _ => String::new(),
        };
        match tool {
            "text.upper" => {
                let shout = params.get("shout") == Some(&Value::Bool(true));
                Ok(ValueMap::from([(
                    "text".to_owned(),
                    Value::Text(if shout {
                        format!("{}!", text.to_uppercase())
                    } else {
                        text.to_uppercase()
                    }),
                )]))
            }
            "text.repeat" => {
                let times = match params.get("times") {
                    Some(Value::Int(n)) => *n,
                    _ => 0,
                };
                Ok(ValueMap::from([
                    (
                        "text".to_owned(),
                        Value::Text(text.repeat(usize::try_from(times).unwrap_or(0))),
                    ),
                    ("count".to_owned(), Value::Int(times)),
                ]))
            }
            "file.sink" => match inputs.get("file") {
                Some(Value::File(f)) => Ok(ValueMap::from([(
                    "size".to_owned(),
                    Value::Int(i64::try_from(f.bytes.len()).unwrap_or(0)),
                )])),
                _ => Err(ToolError::new("missing_input").into()),
            },
            _ => Err(ToolError::new("unknown_tool").into()),
        }
    }
}

#[test]
fn runs_nodes_in_dependency_order_even_when_listed_backwards() {
    let w = workflow(
        vec![
            tool("b", "text.upper"),
            tool("a", "text.repeat"),
            text("src", "ab"),
        ],
        vec![
            edge(("src", "text"), ("a", "text")),
            edge(("a", "text"), ("b", "text")),
        ],
    );
    assert!(validate(&w, &catalogue()).is_empty());
    let fake = Fake::new(0);
    let report = run(&w, &catalogue(), &fake, &CancelToken::default())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(*fake.calls.borrow(), ["text.repeat", "text.upper"]);
    assert_eq!(report.results["b"]["text"], Value::Text("ABAB".into()));
    assert_eq!(report.results["a"]["count"], Value::Int(2));
    assert_eq!(report.journal.len(), 3);
    assert_eq!(order(&w).map(|o| o.len()), Ok(3));
}

#[test]
fn params_are_typed_by_the_contract_and_defaults_apply() {
    let mut w = workflow(
        vec![text("s", "x"), tool("r", "text.repeat")],
        vec![edge(("s", "text"), ("r", "text"))],
    );
    if let NodeKind::Tool { params, .. } = &mut w.nodes[1].kind {
        params.insert("times".into(), "3".into());
    }
    let report = run(&w, &catalogue(), &Fake::new(0), &CancelToken::default())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(report.results["r"]["text"], Value::Text("xxx".into()));
    let specs = &catalogue()["text.repeat"].params;
    let bad = |key: &str, value: &str| {
        param_values(&BTreeMap::from([(key.to_owned(), value.to_owned())]), specs)
            .err()
            .map(|i| (i.code, i.detail))
    };
    assert_eq!(bad("times", "three"), Some(("bad_param", "times".into())));
    assert_eq!(bad("mode", "zigzag"), Some(("bad_param", "mode".into())));
    assert_eq!(
        param_values(&BTreeMap::new(), &catalogue()["text.upper"].params).unwrap_or_default()["shout"],
        Value::Bool(false)
    );
    assert!(
        param_values(
            &BTreeMap::from([("shout".into(), "yes".into())]),
            &catalogue()["text.upper"].params
        )
        .is_err()
    );
}

#[test]
fn validation_reports_each_kind_of_problem() {
    let one = |nodes, edges| codes(&workflow(nodes, edges));
    assert_eq!(
        one(vec![text("a", ""), text("a", "")], vec![]),
        ["duplicate_node"]
    );
    assert_eq!(one(vec![tool("t", "text.nope")], vec![]), ["unknown_tool"]);
    assert_eq!(
        one(
            vec![text("a", "")],
            vec![edge(("ghost", "text"), ("a", "text"))]
        ),
        ["unknown_node"]
    );
    assert_eq!(
        one(
            vec![text("a", ""), tool("u", "text.upper")],
            vec![edge(("a", "nope"), ("u", "text"))]
        ),
        ["unknown_port"]
    );
    assert_eq!(
        one(
            vec![text("a", ""), tool("u", "text.upper")],
            vec![edge(("a", "text"), ("u", "nope"))]
        ),
        ["unknown_port", "missing_input"]
    );
    assert_eq!(
        one(vec![tool("u", "text.upper")], vec![]),
        ["missing_input"]
    );
    assert_eq!(
        one(
            vec![
                tool("r", "text.repeat"),
                tool("u", "text.upper"),
                text("s", "")
            ],
            vec![
                edge(("s", "text"), ("r", "text")),
                edge(("r", "count"), ("u", "text"))
            ]
        ),
        ["type_mismatch"]
    );
    assert_eq!(
        one(
            vec![text("a", ""), text("b", ""), tool("u", "text.upper")],
            vec![
                edge(("a", "text"), ("u", "text")),
                edge(("b", "text"), ("u", "text"))
            ]
        ),
        ["input_taken"]
    );
    assert!(
        one(
            vec![tool("a", "text.upper"), tool("b", "text.upper")],
            vec![
                edge(("a", "text"), ("b", "text")),
                edge(("b", "text"), ("a", "text"))
            ]
        )
        .contains(&"cycle")
    );
}

#[test]
fn an_invalid_workflow_never_reaches_the_runner() {
    let fake = Fake::new(0);
    let failure = run(
        &workflow(vec![tool("u", "text.upper")], vec![]),
        &catalogue(),
        &fake,
        &CancelToken::default(),
    )
    .err();
    assert!(fake.calls.borrow().is_empty());
    let failure = failure.unwrap_or_else(|| unreachable!("must fail"));
    assert_eq!(
        (
            failure.code.as_str(),
            failure.node.as_deref(),
            failure.detail.as_deref()
        ),
        ("invalid_workflow", Some("u"), Some("missing_input"))
    );
}

#[test]
fn text_becomes_a_file_through_a_real_adapter() {
    let w = workflow(
        vec![text("s", "héllo"), tool("f", "file.sink")],
        vec![edge(("s", "text"), ("f", "file"))],
    );
    assert!(
        validate(&w, &catalogue()).is_empty(),
        "Text converts to file<text/plain>"
    );
    let report = run(&w, &catalogue(), &Fake::new(0), &CancelToken::default())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(
        report.results["f"]["size"],
        Value::Int(6),
        "UTF-8 bytes of héllo"
    );
}

#[test]
fn adapters_build_csv_and_wrap_single_files() {
    let table = Table {
        columns: vec!["a".into(), "b,c".into()],
        rows: vec![vec!["1".into(), "say \"hi\"".into()]],
    };
    let Ok(Value::File(csv)) = adapt(Value::Table(table), &PortType::File("text/csv".into()))
    else {
        unreachable!("csv file")
    };
    assert_eq!(
        String::from_utf8_lossy(&csv.bytes),
        "a,\"b,c\"\n1,\"say \"\"hi\"\"\""
    );
    assert_eq!(csv.mime, "text/csv");
    let Ok(Value::Files(files)) = adapt(Value::File(csv), &PortType::Files("*/*".into())) else {
        unreachable!("wrapped")
    };
    assert_eq!(files.len(), 1);
    assert_eq!(
        adapt(Value::Text("t".into()), &PortType::Text),
        Ok(Value::Text("t".into()))
    );
}

#[test]
fn retries_recover_transient_failures_and_then_give_up() {
    let mut w = workflow(
        vec![text("s", "x"), tool("u", "text.upper")],
        vec![edge(("s", "text"), ("u", "text"))],
    );
    w.nodes[1].retries = 2;
    let fake = Fake::new(2);
    let report = run(&w, &catalogue(), &fake, &CancelToken::default())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(fake.calls.borrow().len(), 3);
    assert!(report.journal.iter().any(|l| l.contains("retry 2")));
    let fake = Fake::new(5);
    let failure = run(&w, &catalogue(), &fake, &CancelToken::default())
        .err()
        .unwrap_or_else(|| unreachable!("must fail"));
    assert_eq!(fake.calls.borrow().len(), 3, "1 attempt + 2 retries");
    assert_eq!(
        (failure.code.as_str(), failure.node.as_deref()),
        ("worker_failed", Some("u"))
    );
    assert!(
        failure.report.results.contains_key("s"),
        "completed nodes stay in the report"
    );
}

#[test]
fn cancellation_stops_before_the_next_node() {
    let w = workflow(
        vec![text("s", "x"), tool("u", "text.upper")],
        vec![edge(("s", "text"), ("u", "text"))],
    );
    let cancel = CancelToken::default();
    cancel.cancel();
    let fake = Fake::new(0);
    let failure = run(&w, &catalogue(), &fake, &cancel)
        .err()
        .unwrap_or_else(|| unreachable!("must fail"));
    assert_eq!(failure.code, ToolError::CANCELLED);
    assert!(fake.calls.borrow().is_empty());
}

#[test]
fn files_roundtrip_through_versioned_json_and_reject_bad_input() {
    let w = workflow(
        vec![text("s", "x"), tool("u", "text.upper")],
        vec![edge(("s", "text"), ("u", "text"))],
    );
    let json = to_json(&w);
    assert!(
        json.contains("\"kind\": \"text\"") && json.contains("\"kind\": \"tool\""),
        "{json}"
    );
    assert_eq!(from_json(&json), Ok(w.clone()));
    let newer = json.replace(&format!("\"version\": {FORMAT_VERSION}"), "\"version\": 99");
    assert_eq!(
        from_json(&newer).err().map(|i| i.code),
        Some("too_new_version")
    );
    assert_eq!(
        from_json("{nope").err().map(|i| i.code),
        Some("invalid_json")
    );
    assert_eq!(
        from_json("{\"version\":1,\"name\":\"x\",\"nodes\":[],\"edges\":[]}").map(|w| w.version),
        Ok(FORMAT_VERSION)
    );
}

#[test]
fn table_ports_feed_csv_file_inputs() {
    // Table -> file<text/plain> is not a safe conversion: only csv patterns accept a table.
    let w = workflow(
        vec![tool("t", "table.make"), tool("f", "file.sink")],
        vec![edge(("t", "table"), ("f", "file"))],
    );
    assert!(codes(&w).contains(&"type_mismatch"));
}

#[test]
fn a_host_can_drive_the_run_one_job_at_a_time_and_report_back_later() {
    let w = workflow(
        vec![
            text("s", "x"),
            tool("u", "text.upper"),
            tool("r", "text.repeat"),
        ],
        vec![
            edge(("s", "text"), ("u", "text")),
            edge(("u", "text"), ("r", "text")),
        ],
    );
    let cat = catalogue();
    let cancel = CancelToken::default();
    let mut execution = Execution::new(&w, &cat).unwrap_or_else(|e| unreachable!("{e:?}"));
    // The constant node is resolved without a job; the first job is the upper-casing tool.
    let Step::Run(first) = execution.next(&cancel) else {
        unreachable!("first job")
    };
    assert_eq!(
        (first.node.as_str(), first.tool.as_str()),
        ("u", "text.upper")
    );
    assert_eq!(first.inputs["text"], Value::Text("x".into()));
    // Asking again before reporting returns the same job (an async host may poll).
    assert_eq!(execution.next(&cancel), Step::Run(first.clone()));
    execution.finish(Ok(ValueMap::from([(
        "text".to_owned(),
        Value::Text("X".into()),
    )])));
    let Step::Run(second) = execution.next(&cancel) else {
        unreachable!("second job")
    };
    assert_eq!(
        second.inputs["text"],
        Value::Text("X".into()),
        "data flows from the reported result"
    );
    execution.finish(Ok(ValueMap::from([(
        "text".to_owned(),
        Value::Text("XX".into()),
    )])));
    let Step::Done(report) = execution.next(&cancel) else {
        unreachable!("done")
    };
    assert_eq!(report.results["r"]["text"], Value::Text("XX".into()));
    assert_eq!(report.journal, ["s: ok", "u: ok", "r: ok"]);
}

#[test]
fn the_stepper_retries_a_failed_job_and_then_reports_the_failure() {
    let mut w = workflow(
        vec![text("s", "x"), tool("u", "text.upper")],
        vec![edge(("s", "text"), ("u", "text"))],
    );
    w.nodes[1].retries = 1;
    let cat = catalogue();
    let cancel = CancelToken::default();
    let mut execution = Execution::new(&w, &cat).unwrap_or_else(|e| unreachable!("{e:?}"));
    let Step::Run(job) = execution.next(&cancel) else {
        unreachable!("job")
    };
    execution.finish(Err(RunError {
        code: "worker_failed".into(),
        detail: None,
    }));
    assert_eq!(
        execution.next(&cancel),
        Step::Run(job),
        "one retry allowed: same job again"
    );
    execution.finish(Err(RunError {
        code: "worker_failed".into(),
        detail: Some("again".into()),
    }));
    let Step::Failed(failure) = execution.next(&cancel) else {
        unreachable!("failure")
    };
    assert_eq!(
        (
            failure.code.as_str(),
            failure.node.as_deref(),
            failure.detail.as_deref()
        ),
        ("worker_failed", Some("u"), Some("again"))
    );
    assert!(failure.report.journal.iter().any(|l| l.contains("retry 1")));
}

#[test]
fn cancelling_while_a_job_is_out_stops_at_the_next_request_and_never_retries() {
    let mut w = workflow(
        vec![text("s", "x"), tool("u", "text.upper")],
        vec![edge(("s", "text"), ("u", "text"))],
    );
    w.nodes[1].retries = 3;
    let cat = catalogue();
    let cancel = CancelToken::default();
    let mut execution = Execution::new(&w, &cat).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(matches!(execution.next(&cancel), Step::Run(_)));
    cancel.cancel();
    execution.finish(Err(RunError {
        code: ToolError::CANCELLED.into(),
        detail: None,
    }));
    let Step::Failed(failure) = execution.next(&cancel) else {
        unreachable!("cancelled")
    };
    assert_eq!(failure.code, ToolError::CANCELLED);
    assert!(!failure.report.journal.iter().any(|l| l.contains("retry")));
}

#[test]
fn an_invalid_workflow_cannot_start_a_stepper() {
    let cat = catalogue();
    let w = workflow(vec![tool("u", "text.upper")], vec![]);
    assert_eq!(
        Execution::new(&w, &cat).err().map(|f| f.code),
        Some("invalid_workflow".to_owned())
    );
}
