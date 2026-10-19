//! Properties panel of the selected node: its content, inputs, parameters, retries, chosen files
//! and results. Wiring by list stays here for keyboard users, next to the drag gesture.
use dioxus::prelude::*;
use kortex_contract::{ParamLevel, PortType};
use kortex_runtime::{FileData, Value};
use kortex_shell as shell;

use crate::canvas::{self, can_connect, node_ports};
use crate::catalogue::ports;
use crate::engine::{Endpoint, Node, NodeKind, Workflow};
use crate::ui::{ACCEPTS, Ctx, node_title, summary};

/// Saves a produced or exported file: a browser download, or the desktop downloads folder.
pub(crate) fn save_file(
    name: &str,
    mime: &str,
    bytes: &[u8],
    mut note: Signal<String>,
    l: &kortex_i18n::Catalog,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = l;
        kortex_platform::document::download(name, mime, bytes);
        note.set(String::new());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = mime;
        note.set(match kortex_platform::native::save_download(name, bytes) {
            Ok(path) => format!("{} {path}", l.t("ui.saved")),
            Err(error) => error,
        });
    }
}

/// One editable parameter of a tool or a logic node.
struct Field {
    id: String,
    label: String,
    ty: PortType,
    options: Vec<String>,
    value: String,
}

fn fields(node: &Node, pro: bool, l: &kortex_i18n::Catalog, lang: &str) -> Vec<Field> {
    let (specs, params) = match &node.kind {
        NodeKind::Tool { params, tool } => (
            ports().get(tool).cloned().unwrap_or_default().params,
            params,
        ),
        NodeKind::Logic { params, op } => (
            crate::engine::logic_ports(op).unwrap_or_default().params,
            params,
        ),
        _ => return Vec::new(),
    };
    let entry = match &node.kind {
        NodeKind::Tool { tool, .. } => shell::find(tool),
        _ => None,
    };
    specs
        .into_iter()
        .filter(|spec| {
            entry
                .and_then(|e| e.params.iter().find(|p| p.id == spec.id))
                .is_none_or(|p| pro || p.level != Some(ParamLevel::Pro))
        })
        .map(|spec| {
            let label = match &node.kind {
                NodeKind::Logic { .. } => l.t(&format!("logic.param.{}", spec.id)).to_owned(),
                _ => entry
                    .and_then(|e| e.params.iter().find(|p| p.id == spec.id))
                    .and_then(|p| p.label.as_ref())
                    .and_then(|m| m.get(lang).or_else(|| m.get("en")))
                    .cloned()
                    .unwrap_or_else(|| spec.id.clone()),
            };
            Field {
                value: params
                    .get(&spec.id)
                    .cloned()
                    .unwrap_or_else(|| spec.default.clone()),
                id: spec.id,
                label,
                ty: spec.ty,
                options: spec.options,
            }
        })
        .collect()
}

fn set_param(workflow: &mut Workflow, node: &str, key: &str, text: String) {
    if let Some(n) = workflow.nodes.iter_mut().find(|n| n.id == node)
        && let NodeKind::Tool { params, .. } | NodeKind::Logic { params, .. } = &mut n.kind
    {
        params.insert(key.to_owned(), text);
    }
}

pub(crate) fn inspector(
    node: &Node,
    current: &Workflow,
    ctx: Ctx,
    pro: bool,
    l: &'static kortex_i18n::Catalog,
    lang: &str,
) -> Element {
    let Ctx {
        mut workflow,
        mut selected,
        results,
        note,
        mut files,
        ..
    } = ctx;
    let id = node.id.clone();
    let title = node_title(node, l, lang);
    let retries = node.retries;
    let produced = results.read().get(&node.id).cloned().unwrap_or_default();
    let (remove_id, retry_id) = (id.clone(), id.clone());
    let params = fields(node, pro, l, lang);
    let wired: Vec<_> = node_ports(node, ports()).0;
    let options_label = |kind: &str, option: &str| -> String {
        let key = format!("logic.{kind}.{option}");
        let text = l.t(&key);
        if text == key {
            option.to_owned()
        } else {
            text.to_owned()
        }
    };
    rsx! {
        strong { "{id} · {title}" }
        match node.kind.clone() {
            NodeKind::Text { value } => {
                let text_id = id.clone();
                rsx! {
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("ui.text_node")} }
                        textarea {
                            class: "k-input k-textarea",
                            rows: "6",
                            value: "{value}",
                            onfocus: move |_| ctx.snapshot(),
                            oninput: move |e| {
                                if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == text_id) {
                                    n.kind = NodeKind::Text { value: e.value() };
                                }
                            },
                        }
                    }
                }
            }
            NodeKind::File { accept, multiple } => {
                let (kind_id, pick_id) = (id.clone(), id.clone());
                let accept_now = if accept.is_empty() { "*/*".to_owned() } else { accept.clone() };
                let accept_attr = accept_now.clone();
                let chosen = files.read().get(&id).cloned();
                rsx! {
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("ui.accept")} }
                        select {
                            class: "k-input",
                            "data-field": "accept",
                            value: "{accept_now}",
                            onchange: move |e| {
                                ctx.snapshot();
                                if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == kind_id) {
                                    n.kind = NodeKind::File { accept: e.value(), multiple };
                                }
                            },
                            for (mime , key) in ACCEPTS {
                                option { key: "{mime}", value: "{mime}", {l.t(key)} }
                            }
                        }
                    }
                    label { class: "k-field",
                        span { class: "k-muted", {l.t(if multiple { "ui.pick_files" } else { "ui.pick_file" })} }
                        input {
                            class: "k-input",
                            r#type: "file",
                            "data-field": "file",
                            multiple,
                            accept: if accept_attr == "*/*" { String::new() } else { accept_attr },
                            onchange: move |e| {
                                let picked = e.files();
                                let node = pick_id.clone();
                                spawn(async move {
                                    let mut read: Vec<FileData> = Vec::new();
                                    for file in &picked {
                                        if let Ok(f) = kortex_platform::document::read_selected(file).await {
                                            read.push(FileData { name: f.name, mime: f.mime, bytes: f.bytes });
                                        }
                                    }
                                    let value = if multiple {
                                        (!read.is_empty()).then_some(Value::Files(read))
                                    } else {
                                        read.into_iter().next().map(Value::File)
                                    };
                                    match value {
                                        Some(v) => {
                                            files.write().insert(node, v);
                                        }
                                        None => {
                                            files.write().remove(&node);
                                        }
                                    }
                                });
                            },
                        }
                    }
                    p { class: "flow-hint", "data-field": "file-status",
                        {match &chosen {
                            Some(Value::File(f)) => f.name.clone(),
                            Some(Value::Files(all)) => all.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", "),
                            _ => l.t("ui.no_file").to_owned(),
                        }}
                    }
                    p { class: "flow-hint", {l.t("ui.file_privacy")} }
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("ui.batch")} }
                        select {
                            class: "k-input",
                            "data-field": "multiple",
                            value: "{multiple}",
                            onchange: move |e| {
                                ctx.snapshot();
                                let now = e.value() == "true";
                                let node = id.clone();
                                if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == node) {
                                    n.kind = NodeKind::File { accept: accept.clone(), multiple: now };
                                }
                                // The old output port no longer exists: drop its links and files.
                                let port = if multiple { "files" } else { "file" };
                                workflow.write().edges.retain(|edge| !(edge.from.node == node && edge.from.port == port));
                                files.write().remove(&node);
                            },
                            option { value: "false", {l.t("ui.single")} }
                            option { value: "true", {l.t("ui.multiple")} }
                        }
                    }
                }
            }
            NodeKind::Tool { .. } | NodeKind::Logic { .. } => rsx! {
                for input in wired.clone() {
                    {wire(&node.id, &input.id, input.required, current, ctx, l)}
                }
                for field in params {
                    {
                        let (node_id, key) = (node.id.clone(), field.id.clone());
                        let set = move |text: String| set_param(&mut workflow.write(), &node_id, &key, text);
                        let is_bool = field.ty == PortType::Bool;
                        let is_logic = matches!(node.kind, NodeKind::Logic { .. });
                        let choices: Vec<String> = if is_bool { vec!["true".to_owned(), "false".to_owned()] } else { field.options.clone() };
                        let kind_key = match field.id.as_str() {
                            "keep" => "keep",
                            _ => "test",
                        };
                        let mut set_change = set.clone();
                        let mut set_input = set;
                        rsx! {
                            label { key: "{field.id}", class: "k-field",
                                span { class: "k-muted", "{field.label}" }
                                if !choices.is_empty() {
                                    select {
                                        class: "k-input",
                                        "data-field": "{field.id}",
                                        value: "{field.value}",
                                        onfocus: move |_| ctx.snapshot(),
                                        onchange: move |e| set_change(e.value()),
                                        for option in choices {
                                            option { key: "{option}", value: "{option}",
                                                {if is_logic && !is_bool { options_label(kind_key, &option) } else { option.clone() }}
                                            }
                                        }
                                    }
                                } else {
                                    input {
                                        class: "k-input",
                                        "data-field": "{field.id}",
                                        value: "{field.value}",
                                        onfocus: move |_| ctx.snapshot(),
                                        oninput: move |e| set_input(e.value()),
                                    }
                                }
                            }
                        }
                    }
                }
            },
        }
        if !matches!(node.kind, NodeKind::File { .. } | NodeKind::Text { .. }) || retries > 0 {
            label { class: "k-field",
                span { class: "k-muted", {l.t("ui.retries")} }
                select {
                    class: "k-input",
                    value: "{retries}",
                    onchange: move |e| {
                        ctx.snapshot();
                        if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == retry_id) {
                            n.retries = e.value().parse().unwrap_or(0);
                        }
                    },
                    for n in 0..=3_u8 {
                        option { key: "{n}", value: "{n}", "{n}" }
                    }
                }
            }
        }
        if !produced.is_empty() {
            strong { {l.t("ui.result")} }
            for (port , value) in produced {
                div { key: "{port}",
                    span { class: "k-muted", "{port}" }
                    div { class: "flow-result", "{summary(&value)}" }
                    for file in match value.clone() {
                        Value::File(f) => vec![f],
                        Value::Files(all) => all,
                        _ => Vec::new(),
                    } {
                        button {
                            key: "{file.name}",
                            class: "k-btn",
                            "data-variant": "ghost",
                            r#type: "button",
                            "data-download": "{file.name}",
                            onclick: move |_| save_file(&file.name, &file.mime, &file.bytes, note, l),
                            "{l.t(\"ui.download\")} {file.name}"
                        }
                    }
                }
            }
        }
        button {
            class: "k-btn",
            "data-variant": "ghost",
            r#type: "button",
            "data-remove": "{remove_id}",
            onclick: move |_| {
                ctx.snapshot();
                canvas::remove_node(&mut workflow.write(), &remove_id);
                selected.set(None);
            },
            {l.t("ui.remove")}
        }
    }
}

/// Keyboard-friendly alternative to dragging: pick the upstream output of an input from a list.
fn wire(
    node_id: &str,
    input: &str,
    required: bool,
    current: &Workflow,
    ctx: Ctx,
    l: &kortex_i18n::Catalog,
) -> Element {
    let mut workflow = ctx.workflow;
    let target = Endpoint {
        node: node_id.to_owned(),
        port: input.to_owned(),
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
            node_ports(n, ports())
                .1
                .into_iter()
                .filter(|p| {
                    can_connect(
                        current,
                        ports(),
                        &Endpoint {
                            node: n.id.clone(),
                            port: p.id.clone(),
                        },
                        &target,
                    )
                    .is_ok()
                })
                .map(|p| format!("{}.{}", n.id, p.id))
                .collect::<Vec<_>>()
        })
        .collect();
    let label = format!(
        "{input} ← {}",
        if required {
            l.t("ui.required")
        } else {
            l.t("ui.optional")
        }
    );
    rsx! {
        label { key: "in-{input}", class: "k-field",
            span { class: "k-muted", "{label}" }
            select {
                class: "k-input",
                value: "{chosen}",
                onchange: move |e| {
                    ctx.snapshot();
                    let from = e.value().split_once('.').map(|(node, port)| Endpoint { node: node.to_owned(), port: port.to_owned() });
                    canvas::set_edge(&mut workflow.write(), &target, from);
                },
                option { value: "", "—" }
                for candidate in candidates {
                    option { key: "{candidate}", value: "{candidate}", "{candidate}" }
                }
            }
        }
    }
}
