//! Runs real installed tool binaries through `ProcessRunner` (`--engine` mode). Needs the
//! desktop binaries: `cargo xtask build-desktop text.cleaner text.case-converter text.slug-generator`.
//! Without them the tests print a notice and pass, so a plain `cargo test` stays hermetic.
#![cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use kortex_runtime::{CancelToken, Value};
use kortex_workflow::process::ProcessRunner;
use kortex_workflow::{Edge, Endpoint, Node, NodeKind, Workflow, run};

fn ports() -> &'static std::collections::BTreeMap<String, kortex_workflow::ToolPorts> {
    static PORTS: std::sync::OnceLock<
        std::collections::BTreeMap<String, kortex_workflow::ToolPorts>,
    > = std::sync::OnceLock::new();
    PORTS.get_or_init(|| {
        let registry = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../registry/index.json"
        ))
        .unwrap_or_default();
        kortex_workflow::catalogue_of(
            &kortex_workflow::contracts_from_registry(&registry).unwrap_or_default(),
        )
    })
}

fn binary(id: &str) -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/desktop");
    std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter_map(|target| std::fs::read_dir(target.path().join(id)).ok())
        .flat_map(|files| files.flatten().map(|f| f.path()))
        .find(|path| path.is_file())
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

fn tool(id: &str, tool: &str, params: &[(&str, &str)]) -> Node {
    Node {
        id: id.into(),
        kind: NodeKind::Tool {
            tool: tool.into(),
            params: params
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        },
        retries: 0,
        pos: None,
    }
}

fn runner() -> Option<ProcessRunner> {
    let ids = ["text.cleaner", "text.case-converter", "text.slug-generator"];
    let binaries: BTreeMap<String, PathBuf> = ids
        .iter()
        .filter_map(|id| binary(id).map(|p| ((*id).to_owned(), p)))
        .collect();
    if binaries.len() == ids.len() {
        Some(ProcessRunner {
            binaries,
            timeout: Duration::from_secs(60),
        })
    } else {
        eprintln!("skipped: desktop binaries not built (cargo xtask build-desktop …)");
        None
    }
}

fn workflow(text: &str) -> Workflow {
    Workflow {
        nodes: vec![
            Node {
                id: "n1".into(),
                kind: NodeKind::Text { value: text.into() },
                retries: 0,
                pos: None,
            },
            tool("n2", "text.cleaner", &[]),
            tool("n3", "text.case-converter", &[("case", "snake")]),
            tool("n4", "text.slug-generator", &[]),
        ],
        edges: vec![
            edge(("n1", "text"), ("n2", "text")),
            edge(("n2", "text"), ("n3", "text")),
            edge(("n3", "text"), ("n4", "text")),
        ],
        ..Workflow::default()
    }
}

fn text(report: &kortex_workflow::Report, node: &str) -> String {
    match report.results.get(node).and_then(|r| r.get("text")) {
        Some(Value::Text(t)) => t.clone(),
        other => unreachable!("{node}: {other:?}"),
    }
}

#[test]
fn three_real_tools_run_as_a_chain_through_their_engine_mode() {
    let Some(runner) = runner() else { return };
    let w = workflow("  Hello   Kortex World  ");
    let report = run(&w, ports(), &runner, &CancelToken::default())
        .unwrap_or_else(|f| unreachable!("{f:?}"));
    assert_eq!(text(&report, "n2"), "Hello Kortex World");
    assert_eq!(text(&report, "n3"), "hello_kortex_world");
    let slug = text(&report, "n4");
    assert!(slug.contains("hello") && slug.contains("kortex"), "{slug}");
}

#[test]
fn a_tool_error_stops_the_run_with_its_own_code_and_node() {
    let Some(runner) = runner() else { return };
    let failure = run(&workflow(""), ports(), &runner, &CancelToken::default())
        .err()
        .unwrap_or_else(|| unreachable!("an empty text must fail"));
    assert_eq!(
        (failure.code.as_str(), failure.node.as_deref()),
        ("empty_input", Some("n2"))
    );
    assert!(failure.report.results.contains_key("n1"));
    assert!(!failure.report.results.contains_key("n3"));
}

/// A file chosen in Flow desktop reaches the installed engine byte for byte (PLAN 15.11).
#[test]
fn a_file_value_reaches_a_native_engine_intact() {
    use kortex_runtime::FileData;
    use kortex_workflow::Runner;
    let Some(path) = binary("secure.hash-calculator") else {
        eprintln!("skipped: secure.hash-calculator desktop binary not built");
        return;
    };
    let runner = ProcessRunner {
        binaries: BTreeMap::from([("secure.hash-calculator".to_owned(), path)]),
        timeout: Duration::from_secs(60),
    };
    // Every byte value, so an encoding that is not binary-safe would show.
    let bytes: Vec<u8> = (0..=255).collect();
    let inputs = kortex_runtime::ValueMap::from([(
        "file".to_owned(),
        Value::File(FileData {
            name: "all-bytes.bin".into(),
            mime: "application/octet-stream".into(),
            bytes,
        }),
    )]);
    let params =
        kortex_runtime::ValueMap::from([("algorithm".to_owned(), Value::Text("sha256".into()))]);
    let out = runner
        .run("secure.hash-calculator", inputs, params)
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(
        out.get("hash"),
        Some(&Value::Text(
            "40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880".into()
        ))
    );
}
