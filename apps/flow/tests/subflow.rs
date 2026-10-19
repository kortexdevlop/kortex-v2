//! A saved workflow used as a card: doors, ports and a real nested run (logic cards only, so no
//! tool binary is needed).
use std::collections::BTreeMap;
use std::sync::Mutex;

use kortex_flow::catalogue::{
    doors, ports, set_workflow_tools, workflow_name, workflow_ports, workflow_tools,
};
use kortex_flow::engine::{Edge, Endpoint, Job, Node, NodeKind, Workflow, validate};
use kortex_flow::library::Library;
use kortex_runtime::{Value, ValueMap, block_on};

/// The catalogue of saved workflows is process-wide: the tests that change it take turns.
static SERIAL: Mutex<()> = Mutex::new(());

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

/// input "Nom" -> template "Bonjour {a|upper} !" -> output "message"
fn greeter() -> Workflow {
    Workflow {
        name: "Salutation".into(),
        nodes: vec![
            logic(
                "in",
                "input",
                &[("name", "Nom du client"), ("default", "inconnu")],
            ),
            logic("t", "template", &[("template", "Bonjour {a|upper} !")]),
            logic("out", "output", &[("name", "message")]),
        ],
        edges: vec![
            edge(("in", "text"), ("t", "a")),
            edge(("t", "text"), ("out", "text")),
        ],
        ..Workflow::default()
    }
}

#[test]
fn doors_become_the_ports_of_the_card_with_safe_unique_names() {
    let w = greeter();
    let (inputs, outputs) = doors(&w);
    assert_eq!(inputs, [("nom_du_client".to_owned(), "in".to_owned())]);
    assert_eq!(outputs, [("message".to_owned(), "out".to_owned())]);
    let p = workflow_ports(&w).expect("a workflow with doors is a card");
    assert_eq!(
        p.inputs.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        ["nom_du_client"]
    );
    assert_eq!(
        p.outputs.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        ["message"]
    );
    assert!(p.inputs.iter().all(|i| !i.required));
    // Two doors with the same name get distinct ports; no doors, no card.
    let twin = Workflow {
        nodes: vec![
            logic("a", "input", &[("name", "x")]),
            logic("b", "input", &[("name", "X!")]),
        ],
        ..Workflow::default()
    };
    assert_eq!(
        doors(&twin)
            .0
            .iter()
            .map(|d| d.0.as_str())
            .collect::<Vec<_>>(),
        ["x", "x_2"]
    );
    assert!(workflow_ports(&Workflow::default()).is_none());
    assert!(
        validate(&w, ports()).is_empty(),
        "a workflow with doors is valid on its own"
    );
}

#[test]
fn saved_workflows_with_doors_join_the_catalogue_except_the_one_being_edited() {
    let _turn = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut lib = Library::default();
    lib.save(None, "Salutation", &greeter(), 1, || "w1".to_owned());
    lib.save(
        None,
        "Sans portes",
        &Workflow {
            nodes: vec![logic("t", "template", &[])],
            ..Workflow::default()
        },
        2,
        || "w2".to_owned(),
    );
    lib.save(None, "Autre", &greeter(), 3, || "w3".to_owned());
    let offered = workflow_tools(&lib, Some("w3"));
    assert_eq!(
        offered
            .iter()
            .map(|(id, n, _)| (id.as_str(), n.as_str()))
            .collect::<Vec<_>>(),
        [("workflow.w1", "Salutation")]
    );
    set_workflow_tools(offered.clone());
    assert!(ports().contains_key("workflow.w1"));
    assert_eq!(workflow_name("workflow.w1").as_deref(), Some("Salutation"));
    set_workflow_tools(offered);
    // A card of this kind validates like any tool, and disappears when the workflow is removed.
    let user = Workflow {
        nodes: vec![Node {
            id: "c".into(),
            kind: NodeKind::Tool {
                tool: "workflow.w1".into(),
                params: BTreeMap::new(),
            },
            retries: 0,
            pos: None,
        }],
        ..Workflow::default()
    };
    assert!(validate(&user, ports()).is_empty());
    set_workflow_tools(Vec::new());
    assert_eq!(validate(&user, ports())[0].code, "unknown_tool");
}

#[test]
fn a_saved_workflow_runs_as_a_card_with_the_values_it_is_given() {
    let _turn = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let home = std::env::temp_dir().join(format!("kortex-flow-subflow-{}", std::process::id()));
    std::fs::create_dir_all(&home).expect("home");
    // SAFETY: this test binary runs this single test that touches the process environment.
    unsafe { std::env::set_var("KORTEX_HOME", &home) };
    let mut lib = Library::default();
    lib.save(None, "Salutation", &greeter(), 1, || "w1".to_owned());
    assert!(kortex_shell::save("flow", "library", &lib));
    set_workflow_tools(workflow_tools(&lib, None));
    let job = |inputs: &[(&str, &str)]| Job {
        node: "card".into(),
        tool: "workflow.w1".into(),
        inputs: inputs
            .iter()
            .map(|(k, v)| ((*k).into(), Value::Text((*v).into())))
            .collect::<ValueMap>(),
        params: ValueMap::new(),
    };
    let out = block_on(kortex_flow::subflow::run(job(&[("nom_du_client", "ada")]))).expect("runs");
    assert_eq!(out["message"], Value::Text("Bonjour ADA !".into()));
    let default = block_on(kortex_flow::subflow::run(job(&[]))).expect("runs with the default");
    assert_eq!(
        default["message"],
        Value::Text("Bonjour INCONNU !".into()),
        "no value given: the input's default"
    );
    // A missing workflow is reported, and a workflow that calls itself stops at the depth limit.
    let missing = Job {
        tool: "workflow.nope".into(),
        ..job(&[])
    };
    assert_eq!(
        block_on(kortex_flow::subflow::run(missing))
            .expect_err("missing")
            .code,
        "subflow_missing"
    );
    let mut looping = Workflow {
        nodes: vec![
            logic("o", "output", &[("name", "out")]),
            Node {
                id: "self".into(),
                kind: NodeKind::Tool {
                    tool: "workflow.w2".into(),
                    params: BTreeMap::new(),
                },
                retries: 0,
                pos: None,
            },
        ],
        edges: vec![edge(("self", "out"), ("o", "text"))],
        ..Workflow::default()
    };
    looping.nodes.push(logic("i", "input", &[("name", "in")]));
    lib.save(None, "Boucle", &looping, 2, || "w2".to_owned());
    assert!(kortex_shell::save("flow", "library", &lib));
    set_workflow_tools(workflow_tools(&lib, None));
    let error = block_on(kortex_flow::subflow::run(Job {
        tool: "workflow.w2".into(),
        ..job(&[])
    }))
    .expect_err("loop");
    assert!(
        error.code == "subflow_too_deep"
            || error
                .detail
                .as_deref()
                .is_some_and(|d| d.contains("subflow_too_deep")),
        "{error:?}"
    );
    let _ = std::fs::remove_dir_all(&home);
}
