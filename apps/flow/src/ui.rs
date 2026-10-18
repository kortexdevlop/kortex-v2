//! Kortex Flow UI: a step-by-step workflow editor shared by web and desktop. The graph itself is
//! edited as ordered steps with explicit wiring (a list, not yet a canvas); running a workflow is
//! only offered where tool engines are available (desktop, via installed binaries).
use std::collections::BTreeMap;
use std::sync::OnceLock;

use dioxus::prelude::*;
use kortex_contract::{ParamLevel, PortType};
use kortex_shell::{self as shell, Frame};

use crate::catalogue::{ports, usable_tools};
use crate::engine::{
    Edge, Endpoint, Node, NodeKind, Port, TEXT_PORT, Workflow, from_json, order, to_json, validate,
};

fn locale(lang: &str) -> &'static kortex_i18n::Catalog {
    static FR: OnceLock<kortex_i18n::Catalog> = OnceLock::new();
    static EN: OnceLock<kortex_i18n::Catalog> = OnceLock::new();
    if lang == "fr" {
        FR.get_or_init(|| {
            kortex_i18n::Catalog::from_toml(include_str!("../locales/fr.toml")).unwrap_or_default()
        })
    } else {
        EN.get_or_init(|| {
            kortex_i18n::Catalog::from_toml(include_str!("../locales/en.toml")).unwrap_or_default()
        })
    }
}

fn outputs_of(node: &Node) -> Vec<(String, PortType)> {
    match &node.kind {
        NodeKind::Text { .. } => vec![(TEXT_PORT.to_owned(), PortType::Text)],
        NodeKind::Tool { tool, .. } => ports()
            .get(tool)
            .map(|p| {
                p.outputs
                    .iter()
                    .map(|o| (o.id.clone(), o.ty.clone()))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn next_id(workflow: &Workflow) -> String {
    (1..)
        .map(|n| format!("n{n}"))
        .find(|id| workflow.nodes.iter().all(|node| &node.id != id))
        .unwrap_or_default()
}

fn set_edge(workflow: &mut Workflow, to: &Endpoint, from: Option<Endpoint>) {
    workflow.edges.retain(|e| &e.to != to);
    if let Some(from) = from {
        workflow.edges.push(Edge {
            from,
            to: to.clone(),
        });
    }
}

fn remove_node(workflow: &mut Workflow, id: &str) {
    workflow.nodes.retain(|n| n.id != id);
    workflow
        .edges
        .retain(|e| e.from.node != id && e.to.node != id);
}

#[cfg(not(target_arch = "wasm32"))]
fn summary(value: &kortex_runtime::Value) -> String {
    use kortex_runtime::Value;
    match value {
        Value::Text(t) => t.chars().take(2000).collect(),
        Value::Number(n) => n.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::File(f) => format!("{} ({} B)", f.name, f.bytes.len()),
        Value::Files(files) => format!("{} files", files.len()),
        Value::Table(t) => format!("{} × {}", t.rows.len(), t.columns.len()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
type Outcome = Result<crate::engine::Report, crate::engine::Failure>;

/// Runs the workflow on a background thread through the installed tool binaries.
#[cfg(not(target_arch = "wasm32"))]
fn start(
    workflow: Workflow,
    cancel: kortex_runtime::CancelToken,
) -> kortex_platform::native::task::Slot<Outcome> {
    use crate::engine::{Failure, Report, run};
    use kortex_platform::native::task;
    let slot = task::Slot::new();
    let remote = slot.clone();
    let spawned = task::spawn("kortex-flow-run", move || {
        let binaries = workflow
            .nodes
            .iter()
            .filter_map(|n| match &n.kind {
                NodeKind::Tool { tool, .. } => {
                    shell::installed_binary(tool).map(|path| (tool.clone(), path))
                }
                NodeKind::Text { .. } => None,
            })
            .collect();
        let runner = crate::runner::ProcessRunner {
            binaries,
            timeout: std::time::Duration::from_secs(120),
        };
        remote.complete(run(&workflow, ports(), &runner, &cancel));
    });
    if spawned.is_err() {
        slot.complete(Err(Failure {
            node: None,
            code: "worker_failed".into(),
            detail: Some("thread unavailable".into()),
            report: Report::default(),
        }));
    }
    slot
}

#[component]
pub fn App() -> Element {
    let appearance = shell::use_appearance();
    let lang = (appearance.lang)();
    let l = locale(&lang);
    let pro = (appearance.mode)() == "pro";
    let (loaded, storage_error) = use_hook(|| shell::load::<Workflow>("flow", "workflow"));
    let mut workflow = use_signal(move || loaded.clone());
    let mut tool_choice = use_signal(String::new);
    let mut import_text = use_signal(String::new);
    let mut note = use_signal(String::new);
    let mut result = use_signal(|| None::<(bool, Vec<String>)>);
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut running = use_signal(|| false);
    #[cfg(not(target_arch = "wasm32"))]
    let mut cancel = use_signal(kortex_runtime::CancelToken::default);
    use_effect(move || {
        shell::save("flow", "workflow", &*workflow.read());
    });

    let current = workflow();
    let issues = validate(&current, ports());
    let sequence = order(&current).unwrap_or_else(|_| (0..current.nodes.len()).collect());
    let ordered: Vec<Node> = sequence.iter().map(|i| current.nodes[*i].clone()).collect();
    let tools = usable_tools(&lang);
    let issue_text = |code: &str, detail: &str| {
        let key = format!("issues.{code}");
        let text = l.t(&key);
        if detail.is_empty() {
            text.to_owned()
        } else {
            format!("{text} ({detail})")
        }
    };
    let can_run = cfg!(not(target_arch = "wasm32")) && issues.is_empty() && !running();
    #[cfg(not(target_arch = "wasm32"))]
    let missing: Vec<String> = current
        .nodes
        .iter()
        .filter_map(|n| match &n.kind {
            NodeKind::Tool { tool, .. } if shell::installed_binary(tool).is_none() => {
                Some(tool.clone())
            }
            _ => None,
        })
        .collect();
    #[cfg(target_arch = "wasm32")]
    let missing: Vec<String> = Vec::new();

    rsx! {
        Frame { title: l.t("ui.title"), subtitle: l.t("ui.subtitle"),
            style { {include_str!("flow.css")} }
            if storage_error {
                p { class: "app-note", role: "alert", {l.t("ui.storage_error")} }
            }
            div { class: "flow-bar",
                label { class: "k-field flow-name",
                    span { class: "k-muted", {l.t("ui.name")} }
                    input {
                        class: "k-input",
                        value: "{current.name}",
                        oninput: move |e| workflow.write().name = e.value(),
                    }
                }
                button {
                    class: "k-btn",
                    r#type: "button",
                    onclick: move |_| {
                        let mut w = workflow.write();
                        let id = next_id(&w);
                        w.nodes.push(Node { id, kind: NodeKind::Text { value: String::new() }, retries: 0 });
                    },
                    {l.t("ui.add_text")}
                }
                label { class: "k-field",
                    span { class: "k-muted", {l.t("ui.tool") } }
                    select {
                        class: "k-input",
                        value: "{tool_choice}",
                        onchange: move |e| tool_choice.set(e.value()),
                        option { value: "", {l.t("ui.choose_tool")} }
                        for (id , name) in tools.clone() {
                            option { key: "{id}", value: "{id}", "{name}" }
                        }
                    }
                }
                button {
                    class: "k-btn",
                    r#type: "button",
                    disabled: tool_choice().is_empty(),
                    onclick: move |_| {
                        let tool = tool_choice();
                        let mut w = workflow.write();
                        let id = next_id(&w);
                        w.nodes.push(Node { id, kind: NodeKind::Tool { tool, params: BTreeMap::new() }, retries: 0 });
                        tool_choice.set(String::new());
                    },
                    {l.t("ui.add_tool")}
                }
            }
            if ordered.is_empty() {
                p { class: "app-note", role: "status", {l.t("ui.empty")} }
            }
            ol { class: "flow-steps",
                for node in ordered.clone() {
                    {step(node, current.clone(), workflow, pro, l, &lang)}
                }
            }
            if !issues.is_empty() {
                div { class: "app-note", role: "alert",
                    strong { {l.t("ui.issues")} }
                    ul {
                        for (i , item) in issues.iter().enumerate() {
                            li { key: "{i}",
                                if let Some(node) = &item.node {
                                    "{node} — "
                                }
                                {issue_text(item.code, &item.detail)}
                            }
                        }
                    }
                }
            }
            if cfg!(target_arch = "wasm32") {
                p { class: "app-note", role: "status", {l.t("ui.web_run_unavailable")} }
            }
            if !missing.is_empty() {
                p { class: "app-note", role: "status", "{l.t(\"ui.not_installed\")} {missing.join(\", \")}" }
            }
            div { class: "flow-bar",
                button {
                    class: "k-btn",
                    r#type: "button",
                    disabled: !can_run || !missing.is_empty(),
                    onclick: move |_| {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            running.set(true);
                            result.set(None);
                            let token = kortex_runtime::CancelToken::default();
                            cancel.set(token.clone());
                            let slot = start(workflow(), token);
                            let lines = move |outcome: Outcome| match outcome {
                                Ok(report) => (true, flow_lines(&report, None)),
                                Err(failure) => {
                                    let mut lines = flow_lines(&failure.report, None);
                                    lines.push(format!(
                                        "✗ {} — {}{}",
                                        failure.node.as_deref().unwrap_or("-"),
                                        failure.code,
                                        failure.detail.map(|d| format!(" ({d})")).unwrap_or_default()
                                    ));
                                    (false, lines)
                                }
                            };
                            spawn(async move {
                                if let Some(outcome) = slot.wait().await {
                                    result.set(Some(lines(outcome)));
                                }
                                running.set(false);
                            });
                        }
                    },
                    {l.t("ui.run")}
                }
                {
                    #[cfg(not(target_arch = "wasm32"))]
                    rsx! {
                        if running() {
                            button {
                                class: "k-btn",
                                "data-variant": "ghost",
                                r#type: "button",
                                onclick: move |_| cancel.peek().cancel(),
                                {l.t("ui.cancel")}
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    rsx! {}
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    onclick: move |_| {
                        let bytes = to_json(&workflow.peek()).into_bytes();
                        #[cfg(target_arch = "wasm32")]
                        {
                            kortex_platform::document::download("workflow.json", "application/json", &bytes);
                            note.set(String::new());
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            note.set(match kortex_platform::native::save_download("workflow.json", &bytes) {
                                Ok(path) => format!("{} {path}", locale_text("ui.saved")),
                                Err(error) => error,
                            });
                        }
                    },
                    {l.t("ui.export")}
                }
            }
            if !note().is_empty() {
                p { class: "app-note", role: "status", "{note}" }
            }
            if let Some((ok , lines)) = result() {
                div { class: "app-note", role: if ok { "status" } else { "alert" },
                    strong { if ok { {l.t("ui.run_ok")} } else { {l.t("ui.run_failed")} } }
                    pre { class: "flow-log", "{lines.join(\"\\n\")}" }
                }
            }
            details { class: "flow-import",
                summary { {l.t("ui.import")} }
                textarea {
                    class: "k-input k-textarea",
                    rows: "6",
                    spellcheck: "false",
                    value: "{import_text}",
                    oninput: move |e| import_text.set(e.value()),
                }
                button {
                    class: "k-btn",
                    r#type: "button",
                    disabled: import_text().trim().is_empty(),
                    onclick: move |_| match from_json(&import_text()) {
                        Ok(imported) => {
                            workflow.set(imported);
                            import_text.set(String::new());
                            result.set(None);
                            note.set(String::new());
                        }
                        Err(issue) => note.set(issue_text(issue.code, &issue.detail)),
                    },
                    {l.t("ui.import_apply")}
                }
            }
        }
    }
}

/// Texts needed outside a component body (desktop save message).
#[cfg(not(target_arch = "wasm32"))]
fn locale_text(key: &str) -> String {
    let lang = kortex_platform::native::env::var("KORTEX_LANG")
        .ok()
        .filter(|l| l.starts_with("fr"))
        .map_or("en", |_| "fr");
    locale(lang).t(key).to_owned()
}

#[cfg(not(target_arch = "wasm32"))]
fn flow_lines(report: &crate::engine::Report, only: Option<&str>) -> Vec<String> {
    report
        .results
        .iter()
        .filter(|(id, _)| only.is_none_or(|o| o == id.as_str()))
        .flat_map(|(id, outputs)| {
            outputs
                .iter()
                .map(move |(port, value)| format!("{id}.{port} = {}", summary(value)))
        })
        .collect()
}

fn step(
    node: Node,
    current: Workflow,
    mut workflow: Signal<Workflow>,
    pro: bool,
    l: &'static kortex_i18n::Catalog,
    lang: &str,
) -> Element {
    let id = node.id.clone();
    let title = match &node.kind {
        NodeKind::Text { .. } => l.t("ui.text_node").to_owned(),
        NodeKind::Tool { tool, .. } => {
            shell::find(tool).map_or_else(|| tool.clone(), |e| shell::name(e, lang))
        }
    };
    let retries = node.retries;
    let remove_id = id.clone();
    let retry_id = id.clone();
    rsx! {
        li { key: "{id}", class: "flow-step",
            div { class: "flow-step-head",
                strong { "{id} · {title}" }
                label { class: "flow-retries",
                    span { class: "k-muted", {l.t("ui.retries")} }
                    select {
                        class: "k-input",
                        value: "{retries}",
                        onchange: move |e| {
                            if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == retry_id) {
                                n.retries = e.value().parse().unwrap_or(0);
                            }
                        },
                        for n in 0..=3_u8 {
                            option { key: "{n}", value: "{n}", "{n}" }
                        }
                    }
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    onclick: move |_| remove_node(&mut workflow.write(), &remove_id),
                    {l.t("ui.remove")}
                }
            }
            match node.kind.clone() {
                NodeKind::Text { value } => {
                    let text_id = id.clone();
                    rsx! {
                        textarea {
                            class: "k-input k-textarea",
                            rows: "3",
                            "aria-label": l.t("ui.text_node"),
                            value: "{value}",
                            oninput: move |e| {
                                if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == text_id) {
                                    n.kind = NodeKind::Text { value: e.value() };
                                }
                            },
                        }
                    }
                }
                NodeKind::Tool { tool, params } => {
                    let entry = shell::find(&tool);
                    let specs = ports().get(&tool).cloned().unwrap_or_default();
                    let visible: Vec<_> = entry
                        .map(|e| e.params.iter().filter(|p| pro || p.level != Some(ParamLevel::Pro)).cloned().collect())
                        .unwrap_or_default();
                    rsx! {
                        for input in specs.inputs.clone() {
                            {wire(&id, &input, &current, workflow, l)}
                        }
                        for param in visible {
                            {
                                let spec = specs.params.iter().find(|s| s.id == param.id).cloned();
                                let value = params
                                    .get(&param.id)
                                    .cloned()
                                    .or_else(|| spec.as_ref().map(|s| s.default.clone()))
                                    .unwrap_or_default();
                                let label = param.label.as_ref().and_then(|m| m.get(lang).or_else(|| m.get("en"))).cloned().unwrap_or_else(|| param.id.clone());
                                let (node_id, key) = (id.clone(), param.id.clone());
                                let mut set = move |text: String| {
                                    if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == node_id)
                                        && let NodeKind::Tool { params, .. } = &mut n.kind
                                    {
                                        params.insert(key.clone(), text);
                                    }
                                };
                                let options = spec.as_ref().map(|s| s.options.clone()).unwrap_or_default();
                                let is_bool = param.ty == PortType::Bool;
                                rsx! {
                                    label { key: "{param.id}", class: "k-field",
                                        span { class: "k-muted", "{label}" }
                                        if !options.is_empty() || is_bool {
                                            select {
                                                class: "k-input",
                                                value: "{value}",
                                                onchange: move |e| set(e.value()),
                                                for option in if is_bool { vec!["true".to_owned(), "false".to_owned()] } else { options } {
                                                    option { key: "{option}", value: "{option}", "{option}" }
                                                }
                                            }
                                        } else {
                                            input {
                                                class: "k-input",
                                                value: "{value}",
                                                oninput: move |e| set(e.value()),
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Selector wiring one input port to an upstream output of a compatible type.
fn wire(
    node_id: &str,
    input: &Port,
    current: &Workflow,
    mut workflow: Signal<Workflow>,
    l: &'static kortex_i18n::Catalog,
) -> Element {
    let target = Endpoint {
        node: node_id.to_owned(),
        port: input.id.clone(),
    };
    let chosen = current
        .edges
        .iter()
        .find(|e| e.to == target)
        .map(|e| format!("{}.{}", e.from.node, e.from.port))
        .unwrap_or_default();
    let candidates: Vec<String> = current
        .nodes
        .iter()
        .filter(|n| n.id != node_id)
        .flat_map(|n| {
            outputs_of(n)
                .into_iter()
                .filter(|(_, ty)| ty.converts_to(&input.ty))
                .map(|(port, _)| format!("{}.{port}", n.id))
                .collect::<Vec<_>>()
        })
        .collect();
    let label = format!(
        "{} ← {}",
        input.id,
        if input.required {
            l.t("ui.required")
        } else {
            l.t("ui.optional")
        }
    );
    rsx! {
        label { key: "in-{input.id}", class: "k-field",
            span { class: "k-muted", "{label}" }
            select {
                class: "k-input",
                value: "{chosen}",
                onchange: move |e| {
                    let from = e.value().split_once('.').map(|(node, port)| Endpoint { node: node.to_owned(), port: port.to_owned() });
                    set_edge(&mut workflow.write(), &target, from);
                },
                option { value: "", "—" }
                for candidate in candidates {
                    option { key: "{candidate}", value: "{candidate}", "{candidate}" }
                }
            }
        }
    }
}
