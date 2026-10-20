//! "Describe it, get a workflow": the model is shown the catalogue (tool ids, ports, parameters)
//! and answers with a small JSON graph; everything it says is then checked by the same validation
//! as a hand-made workflow, with one repair round that feeds the problems back. The result is
//! only ever opened in the editor, never run: the person looks at it first.
use std::collections::BTreeMap;
use std::future::Future;

use kortex_ai::chat::Message;
use kortex_workflow::{
    Edge, Endpoint, LOGIC_OPS, Node, NodeKind, ToolPorts, Workflow, auto_layout, logic_ports,
    validate,
};
use serde_json::Value as Json;

use crate::catalogue::Catalog;

/// Most nodes a built workflow may have.
pub const MAX_NODES: usize = 14;
/// Repair rounds after the first answer: the problems are fed back this many times at most.
pub const REPAIRS: usize = 2;
/// Longest request sent (characters).
pub const MAX_REQUEST: usize = 1_000;

/// Why a workflow could not be built (`builder.err_<code>` in the locales).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: &'static str,
    pub detail: String,
}

/// A workflow the model proposed, valid for the catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub workflow: Workflow,
    /// Whether the first answer needed a repair round.
    pub repaired: bool,
}

fn type_name(ports: &ToolPorts) -> String {
    let inputs: Vec<String> = ports
        .inputs
        .iter()
        .map(|p| format!("{}:{}{}", p.id, p.ty, if p.required { "*" } else { "" }))
        .collect();
    let params: Vec<String> = ports
        .params
        .iter()
        .map(|p| {
            if p.options.is_empty() {
                format!("{}={}", p.id, p.default)
            } else {
                format!("{}={}[{}]", p.id, p.default, p.options.join("|"))
            }
        })
        .collect();
    let outputs: Vec<String> = ports
        .outputs
        .iter()
        .map(|p| format!("{}:{}", p.id, p.ty))
        .collect();
    format!(
        "in({}) params({}) out({})",
        inputs.join(","),
        params.join(", "),
        outputs.join(",")
    )
}

/// The catalogue as the model sees it: one line per usable card. Saved workflows, connectors and
/// the HTTP card are left out (they need secrets or the person's own setup).
#[must_use]
pub fn catalogue_text(catalog: &Catalog) -> String {
    let mut lines = vec!["TOOLS (type \"tool\"):".to_owned()];
    for (id, ports) in catalog.iter() {
        if id.starts_with("workflow.")
            || id.starts_with("connector.")
            || id == kortex_workflow::http::HTTP_TOOL
        {
            continue;
        }
        lines.push(format!("- {id}: {}", type_name(ports)));
    }
    lines.push("LOGIC (type \"logic\", field \"op\"):".to_owned());
    for op in LOGIC_OPS {
        if op == "http" {
            continue;
        }
        if let Some(ports) = logic_ports(op) {
            lines.push(format!("- {op}: {}", type_name(&ports)));
        }
    }
    lines.join("\n")
}

/// The instruction for the model.
#[must_use]
pub fn instruction(catalog: &Catalog, lang: &str) -> String {
    let language = if lang == "fr" { "French" } else { "English" };
    format!(
        "You build workflows for Kortex Flow from a person's request. Reply with ONE JSON object and nothing else.\n\
Shape: {{\"name\": \"short title in {language}\", \"nodes\": [...], \"edges\": [...]}}.\n\
Nodes: {{\"id\":\"n1\",\"type\":\"text\",\"value\":\"constant text\"}}, {{\"id\":\"n2\",\"type\":\"tool\",\"tool\":\"<tool id>\",\"params\":{{}}}}, {{\"id\":\"n3\",\"type\":\"logic\",\"op\":\"<op>\",\"params\":{{}}}}, {{\"id\":\"n4\",\"type\":\"file\",\"accept\":\"text/*\"}}. Built-in cards are always type \"logic\" with an \"op\", e.g. {{\"id\":\"n5\",\"type\":\"logic\",\"op\":\"input\",\"params\":{{\"name\":\"Text\"}}}} or {{\"id\":\"n6\",\"type\":\"logic\",\"op\":\"output\",\"params\":{{\"name\":\"Result\"}}}}.\n\
Edges: {{\"from\":\"n1.text\",\"to\":\"n2.text\"}} (node id, dot, port id). Connect an output port to an input port of the SAME type only (a number or a table cannot feed a text input); when types differ, use another port or leave that value out.\n\
Rules: at most {MAX_NODES} nodes; use only the ids, ports and parameters listed below; parameter values are strings; every input marked * must be connected or be a parameter; use an \"input\" logic card (param name) for something the person gives at run time and an \"output\" card (param name) for the result; never invent a tool.\n\
{}",
        catalogue_text(catalog)
    )
}

fn failure(code: &'static str, detail: impl Into<String>) -> Failure {
    Failure {
        code,
        detail: detail.into(),
    }
}

fn param_map(value: Option<&Json>) -> BTreeMap<String, String> {
    value
        .and_then(Json::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| {
                    let text = match v {
                        Json::String(s) => s.clone(),
                        Json::Number(n) => n.to_string(),
                        Json::Bool(b) => b.to_string(),
                        _ => return None,
                    };
                    Some((k.clone(), text))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn endpoint(text: &str) -> Option<Endpoint> {
    let (node, port) = text.split_once('.')?;
    (!node.is_empty() && !port.is_empty()).then(|| Endpoint {
        node: node.to_owned(),
        port: port.to_owned(),
    })
}

/// The JSON object inside a reply, with or without a code fence or a sentence around it.
fn json_in(reply: &str) -> Option<&str> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    (end > start).then(|| &reply[start..=end])
}

/// Reads the model's reply into a workflow (not yet validated against the catalogue).
///
/// # Errors
/// `not_json`, `empty`, `too_big`, or `bad_node` / `bad_edge` with what was wrong.
pub fn parse(reply: &str) -> Result<Workflow, Failure> {
    let text = json_in(reply).ok_or_else(|| failure("not_json", "no JSON object"))?;
    let root: Json = serde_json::from_str(text).map_err(|e| failure("not_json", e.to_string()))?;
    let nodes = root
        .get("nodes")
        .and_then(Json::as_array)
        .ok_or_else(|| failure("empty", "no nodes"))?;
    if nodes.is_empty() {
        return Err(failure("empty", "no nodes"));
    }
    if nodes.len() > MAX_NODES {
        return Err(failure("too_big", format!("{} nodes", nodes.len())));
    }
    let mut workflow = Workflow {
        name: root
            .get("name")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .chars()
            .take(80)
            .collect(),
        ..Workflow::default()
    };
    let text_of = |n: &Json, key: &str| n.get(key).and_then(Json::as_str).map(str::to_owned);
    for n in nodes {
        let id = text_of(n, "id").filter(|s| !s.is_empty() && !s.contains('.'));
        let kind = text_of(n, "type");
        let (Some(id), Some(kind)) = (id, kind) else {
            return Err(failure("bad_node", n.to_string()));
        };
        let kind = match kind.as_str() {
            "text" => NodeKind::Text {
                value: text_of(n, "value").unwrap_or_default(),
            },
            "tool" => NodeKind::Tool {
                tool: text_of(n, "tool").ok_or_else(|| failure("bad_node", id.clone()))?,
                params: param_map(n.get("params")),
            },
            "logic" => NodeKind::Logic {
                op: text_of(n, "op").ok_or_else(|| failure("bad_node", id.clone()))?,
                params: param_map(n.get("params")),
            },
            "file" => NodeKind::File {
                accept: text_of(n, "accept").unwrap_or_default(),
                multiple: n.get("multiple").and_then(Json::as_bool).unwrap_or(false),
            },
            // A built-in card written as its own type ("input", "template"…) is understood.
            op if LOGIC_OPS.contains(&op) => NodeKind::Logic {
                op: op.to_owned(),
                params: param_map(n.get("params")),
            },
            other => {
                return Err(failure(
                    "bad_node",
                    format!(
                        "{id}: unknown type \"{other}\"; types are text, tool, logic (with \"op\"), file"
                    ),
                ));
            }
        };
        workflow.nodes.push(Node {
            id,
            kind,
            retries: 0,
            pos: None,
        });
    }
    for e in root
        .get("edges")
        .and_then(Json::as_array)
        .map_or(&[][..], Vec::as_slice)
    {
        let from = text_of(e, "from").and_then(|s| endpoint(&s));
        let to = text_of(e, "to").and_then(|s| endpoint(&s));
        let (Some(from), Some(to)) = (from, to) else {
            return Err(failure("bad_edge", e.to_string()));
        };
        workflow.edges.push(Edge { from, to });
    }
    Ok(workflow)
}

/// What to tell the model when its workflow did not validate: the problems, one per line.
fn feedback(problems: &[String]) -> String {
    format!(
        "That workflow has problems:\n{}\nReply again with the corrected JSON object only.",
        problems.join("\n")
    )
}

fn check(reply: &str, catalog: &Catalog) -> Result<Workflow, Vec<String>> {
    let mut workflow = parse(reply).map_err(|f| vec![format!("{}: {}", f.code, f.detail)])?;
    // A tool the catalogue does not know is a hallucination, even when it looks plausible.
    let issues = validate(&workflow, catalog);
    if issues.is_empty() {
        auto_layout(&mut workflow);
        return Ok(workflow);
    }
    Err(issues
        .iter()
        .take(8)
        .map(|i| {
            format!(
                "{}{}: {}{}",
                i.node
                    .as_deref()
                    .map(|n| format!("node {n} "))
                    .unwrap_or_default(),
                i.code,
                i.detail,
                hint(i.code)
            )
        })
        .collect())
}

/// What to do about a problem, in the model's terms.
fn hint(code: &str) -> &'static str {
    match code {
        "type_mismatch" => {
            " (ports of different types cannot be connected: delete that edge, or use a tool whose output is text)"
        }
        "input_taken" => " (an input takes one connection only: use a merge or template card)",
        "unknown_tool" => " (use only the tool ids listed)",
        _ => "",
    }
}

/// Builds a workflow from a sentence. `ask` sends the messages to the model and returns its text
/// (injected, so the same loop runs against the gateway and against scripted replies in tests).
///
/// # Errors
/// `empty_request`, the transport's own error code, or `invalid` when the repaired answer is
/// still wrong (the detail names the first problem).
pub async fn build<F, Fut>(
    request: &str,
    lang: &str,
    catalog: &Catalog,
    mut ask: F,
) -> Result<Built, Failure>
where
    F: FnMut(Vec<Message>) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    let request: String = request.trim().chars().take(MAX_REQUEST).collect();
    if request.is_empty() {
        return Err(failure("empty_request", ""));
    }
    let mut messages = vec![
        Message::System(instruction(catalog, lang)),
        // The request is the person's words, shown as data.
        Message::User(format!("REQUEST:\n{request}")),
    ];
    let mut last = Vec::new();
    for round in 0..=REPAIRS {
        let reply = ask(messages.clone())
            .await
            .map_err(|code| failure("transport", code))?;
        match check(&reply, catalog) {
            Ok(mut workflow) => {
                if workflow.name.trim().is_empty() {
                    workflow.name = request.chars().take(40).collect();
                }
                return Ok(Built {
                    workflow,
                    repaired: round > 0,
                });
            }
            Err(problems) => {
                messages.push(Message::Assistant {
                    content: Some(reply),
                    tool_calls: Vec::new(),
                });
                messages.push(Message::User(feedback(&problems)));
                last = problems;
            }
        }
    }
    Err(failure(
        "invalid",
        last.into_iter().next().unwrap_or_default(),
    ))
}
