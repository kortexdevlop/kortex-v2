//! Turning the model's tool calls into validated, locally resolved runs, and recording them as a
//! Flow workflow. The model only ever names data by reference; this module resolves references to
//! the real values (attachments, earlier results) and refuses anything the contract does not allow.
use std::collections::BTreeMap;

use kortex_contract::{PortType, ToolContract};
use kortex_runtime::{FileData, Value, ValueMap};
use kortex_workflow::{Edge, Endpoint, FORMAT_VERSION, Node, NodeKind, TEXT_PORT, Workflow, adapt};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::chat::ToolCall;
use crate::consent::{Attachment, Kind};
use crate::tools::tool_id;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reference {
    /// `@texteN`
    Text(usize),
    /// `@fichierN`
    File(usize),
    /// `@cN.port`
    Result { call: String, port: String },
}

#[must_use]
pub fn parse_reference(text: &str) -> Option<Reference> {
    let rest = text.trim().strip_prefix('@')?;
    let number = |s: &str| s.parse::<usize>().ok().filter(|n| *n > 0);
    if let Some(n) = rest.strip_prefix("texte").and_then(number) {
        return Some(Reference::Text(n));
    }
    if let Some(n) = rest.strip_prefix("fichier").and_then(number) {
        return Some(Reference::File(n));
    }
    let (call, port) = rest.split_once('.')?;
    let valid_call = call.strip_prefix('c').and_then(number).is_some();
    (valid_call
        && !port.is_empty()
        && port
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    .then(|| Reference::Result {
        call: call.to_owned(),
        port: port.to_owned(),
    })
}

/// What a call can refer to.
pub struct Context<'a> {
    pub attachments: &'a [Attachment],
    /// Outputs of calls already run, by call id (`c1`).
    pub results: &'a BTreeMap<String, ValueMap>,
    pub contracts: &'a BTreeMap<String, ToolContract>,
}

impl Context<'_> {
    /// Every reference the model may use right now, for the error feedback.
    #[must_use]
    pub fn valid_references(&self) -> Vec<String> {
        let mut refs: Vec<String> = self.attachments.iter().map(|a| a.reference("fr")).collect();
        for (call, outputs) in self.results {
            refs.extend(outputs.keys().map(|port| format!("@{call}.{port}")));
        }
        refs
    }
}

/// Why a call was refused. `code` is stable; `detail` names the culprit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError {
    pub code: &'static str,
    pub detail: String,
}

fn error(code: &'static str, detail: impl Into<String>) -> PlanError {
    PlanError {
        code,
        detail: detail.into(),
    }
}

impl PlanError {
    /// The message sent back to the model as the tool result so it can correct itself.
    #[must_use]
    pub fn feedback(&self, valid: &[String]) -> String {
        let refs = if valid.is_empty() {
            "aucune".to_owned()
        } else {
            valid.join(", ")
        };
        format!(
            "Erreur ({}): {}. Références valides : {refs}. Corrige l'appel.",
            self.code, self.detail
        )
    }
}

/// A call that passed validation, with real values ready to run.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedCall {
    pub call_id: String,
    pub tool: String,
    pub inputs: ValueMap,
    pub params: ValueMap,
    /// Which reference fed which input port.
    pub refs: Vec<(String, Reference)>,
    /// Extra confirmations the user must give: `sensitive`, `network`, `write`.
    pub confirmations: Vec<&'static str>,
}

fn resolve(reference: &Reference, ctx: &Context<'_>) -> Result<Value, PlanError> {
    match reference {
        Reference::Text(n) | Reference::File(n) => {
            let wanted = if matches!(reference, Reference::Text(_)) {
                Kind::Text
            } else {
                Kind::File
            };
            let attachment = ctx
                .attachments
                .iter()
                .find(|a| a.n == *n && a.kind == wanted)
                .ok_or_else(|| error("unknown_reference", reference_text(reference)))?;
            Ok(match attachment.kind {
                Kind::Text => Value::Text(attachment.as_text().unwrap_or_default().to_owned()),
                Kind::File => Value::File(FileData {
                    name: attachment.name.clone(),
                    mime: attachment.mime.clone(),
                    bytes: attachment.bytes.clone(),
                }),
            })
        }
        Reference::Result { call, port } => ctx
            .results
            .get(call)
            .and_then(|outputs| outputs.get(port))
            .cloned()
            .ok_or_else(|| error("unknown_reference", reference_text(reference))),
    }
}

fn reference_text(reference: &Reference) -> String {
    match reference {
        Reference::Text(n) => format!("@texte{n}"),
        Reference::File(n) => format!("@fichier{n}"),
        Reference::Result { call, port } => format!("@{call}.{port}"),
    }
}

fn input_value(
    port: &str,
    ty: &PortType,
    argument: &Json,
    ctx: &Context<'_>,
    refs: &mut Vec<(String, Reference)>,
) -> Result<Value, PlanError> {
    let one = |item: &Json, refs: &mut Vec<(String, Reference)>| -> Result<Value, PlanError> {
        let text = item
            .as_str()
            .ok_or_else(|| error("invalid_argument", port))?;
        if let Some(reference) = parse_reference(text) {
            let value = resolve(&reference, ctx)?;
            refs.push((port.to_owned(), reference));
            return Ok(value);
        }
        // Something shaped like a reference but not one (`@call_0.text`): the model invented it.
        // Treating it as literal text would feed the tool the reference itself.
        if text.starts_with('@') && text.len() > 1 && !text.contains(char::is_whitespace) {
            return Err(error("unknown_reference", text));
        }
        match ty {
            PortType::Text | PortType::RichText => Ok(Value::Text(text.to_owned())),
            _ => Err(error("reference_required", port)),
        }
    };
    let adapted =
        |value: Value, to: &PortType| adapt(value, to).map_err(|_| error("type_mismatch", port));
    match ty {
        PortType::Files(_) => {
            let items: Vec<&Json> = argument
                .as_array()
                .map_or_else(|| vec![argument], |a| a.iter().collect());
            let mut files = Vec::new();
            for item in items {
                match adapted(one(item, refs)?, &PortType::File("*/*".into()))? {
                    Value::File(f) => files.push(f),
                    Value::Files(more) => files.extend(more),
                    _ => return Err(error("type_mismatch", port)),
                }
            }
            Ok(Value::Files(files))
        }
        _ => {
            let value = one(argument, refs)?;
            let ok = matches!(
                (&value, ty),
                (
                    Value::Text(_),
                    PortType::Text | PortType::RichText | PortType::File(_)
                ) | (Value::File(_), PortType::File(_))
            );
            if !ok {
                return Err(error("type_mismatch", port));
            }
            adapted(value, ty)
        }
    }
}

fn param_value(spec: &kortex_contract::Param, argument: &Json) -> Result<Value, PlanError> {
    let bad = || error("invalid_param", spec.id.clone());
    let value = match spec.ty {
        PortType::Integer => Value::Int(argument.as_i64().ok_or_else(bad)?),
        PortType::Number => Value::Number(argument.as_f64().ok_or_else(bad)?),
        PortType::Bool => Value::Bool(argument.as_bool().ok_or_else(bad)?),
        _ => Value::Text(argument.as_str().ok_or_else(bad)?.to_owned()),
    };
    if let (PortType::Number | PortType::Integer, Some([min, max])) =
        (&spec.ty, spec.range.as_deref())
    {
        let n = match &value {
            Value::Int(i) => *i as f64,
            Value::Number(n) => *n,
            _ => 0.0,
        };
        if n < *min || n > *max {
            return Err(error(
                "invalid_param",
                format!("{} ({min}..{max})", spec.id),
            ));
        }
    }
    if let (PortType::Enum, Some(options), Value::Text(text)) = (&spec.ty, &spec.options, &value)
        && !options.contains(text)
    {
        return Err(error(
            "invalid_param",
            format!("{} ({})", spec.id, options.join("|")),
        ));
    }
    Ok(value)
}

/// Validates one tool call and resolves its references to real values.
///
/// # Errors
/// [`PlanError`] for an unknown tool, malformed or unknown arguments, a missing required input, a
/// reference that does not exist, a type that cannot be adapted or an invalid parameter.
pub fn plan_call(call: &ToolCall, ctx: &Context<'_>) -> Result<PlannedCall, PlanError> {
    let id = tool_id(&call.name);
    let contract = ctx
        .contracts
        .get(&id)
        .ok_or_else(|| error("unknown_tool", id.clone()))?;
    let arguments: Json = serde_json::from_str(if call.arguments.trim().is_empty() {
        "{}"
    } else {
        &call.arguments
    })
    .map_err(|_| error("invalid_arguments", "JSON"))?;
    let object = arguments
        .as_object()
        .ok_or_else(|| error("invalid_arguments", "JSON object"))?;
    let known = |key: &str| {
        contract.inputs.iter().any(|i| i.id == key) || contract.params.iter().any(|p| p.id == key)
    };
    if let Some(key) = object.keys().find(|k| !known(k)) {
        return Err(error("unknown_argument", key.clone()));
    }
    let mut inputs = ValueMap::new();
    let mut refs = Vec::new();
    for input in &contract.inputs {
        match object.get(&input.id) {
            Some(argument) => {
                inputs.insert(
                    input.id.clone(),
                    input_value(&input.id, &input.ty, argument, ctx, &mut refs)?,
                );
            }
            None if input.required => return Err(error("missing_input", input.id.clone())),
            None => {}
        }
    }
    let mut params = ValueMap::new();
    for spec in &contract.params {
        if let Some(argument) = object.get(&spec.id) {
            params.insert(spec.id.clone(), param_value(spec, argument)?);
        }
    }
    let permissions = &contract.permissions;
    let mut confirmations = Vec::new();
    if permissions.sensitive {
        confirmations.push("sensitive");
    }
    if permissions.network {
        confirmations.push("network");
    }
    if permissions.filesystem == "write" {
        confirmations.push("write");
    }
    Ok(PlannedCall {
        call_id: call.id.clone(),
        tool: id,
        inputs,
        params,
        refs,
        confirmations,
    })
}

/// What goes back to the model as a tool result: references and sizes, never the data itself.
#[must_use]
pub fn summarize(call_id: &str, outputs: &ValueMap) -> String {
    let mut summary = serde_json::Map::new();
    for (port, value) in outputs {
        let reference = format!("@{call_id}.{port}");
        let text = match value {
            Value::Text(t) => format!("{reference} (texte de {} caractères)", t.chars().count()),
            Value::File(f) => format!(
                "{reference} (fichier {}, {}, {} octets)",
                f.name,
                f.mime,
                f.bytes.len()
            ),
            Value::Files(files) => format!("{reference} ({} fichiers)", files.len()),
            Value::Table(t) => format!(
                "{reference} (tableau {} lignes × {} colonnes)",
                t.rows.len(),
                t.columns.len()
            ),
            Value::Int(i) => i.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
        };
        summary.insert(port.clone(), Json::String(text));
    }
    Json::Object(summary).to_string()
}

/// A call that ran, for the workflow export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Executed {
    pub call_id: String,
    pub tool: String,
    pub params: ValueMap,
    pub refs: Vec<(String, Reference)>,
}

fn param_text(value: &Value) -> String {
    match value {
        Value::Text(t) => t.clone(),
        Value::Int(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => format!("{other:?}"),
    }
}

/// Mime pattern covering every attachment of a batch: their own type when equal, `type/*` when
/// only the family is shared, any otherwise (empty when nothing is known).
fn common_mime(ns: &[usize], attachments: &[Attachment]) -> String {
    let mimes: Vec<&str> = ns
        .iter()
        .filter_map(|n| {
            attachments
                .iter()
                .find(|a| a.n == *n && a.kind == Kind::File)
                .map(|a| a.mime.as_str())
        })
        .collect();
    match mimes.split_first() {
        None => String::new(),
        Some((first, rest)) if rest.iter().all(|m| m == first) => (*first).to_owned(),
        Some((first, rest)) => {
            let family = first.split('/').next().unwrap_or("");
            if !family.is_empty() && rest.iter().all(|m| m.split('/').next() == Some(family)) {
                format!("{family}/*")
            } else {
                "*/*".to_owned()
            }
        }
    }
}

/// The executed calls as a Flow workflow: one tool node per call (`c1`…), one text node per text
/// attachment used (`t1`…), one file node per file attachment (`f1`…) or per batch of files given
/// to the same input (`b1`…), and an edge for every reference. File nodes keep only the accepted
/// type: the user picks the files again in Flow, they are never written into the workflow.
#[must_use]
pub fn to_workflow(name: &str, executed: &[Executed], attachments: &[Attachment]) -> Workflow {
    let mut workflow = Workflow {
        version: FORMAT_VERSION,
        name: name.to_owned(),
        ..Workflow::default()
    };
    let mut used_texts: Vec<usize> = Vec::new();
    for call in executed {
        for (_, reference) in &call.refs {
            if let Reference::Text(n) = reference
                && !used_texts.contains(n)
            {
                used_texts.push(*n);
            }
        }
    }
    for n in &used_texts {
        let value = attachments
            .iter()
            .find(|a| a.n == *n && a.kind == Kind::Text)
            .and_then(Attachment::as_text)
            .unwrap_or_default();
        workflow.nodes.push(Node {
            id: format!("t{n}"),
            kind: NodeKind::Text {
                value: value.to_owned(),
            },
            retries: 0,
            pos: None,
        });
    }
    // File inputs: several files on one input form a batch node, a lone file a single-file node.
    let mut file_nodes: Vec<(Vec<usize>, String)> = Vec::new();
    for call in executed {
        let mut by_port: Vec<(&str, Vec<usize>)> = Vec::new();
        for (port, reference) in &call.refs {
            if let Reference::File(n) = reference {
                match by_port.iter_mut().find(|(p, _)| p == port) {
                    Some((_, ns)) => ns.push(*n),
                    None => by_port.push((port, vec![*n])),
                }
            }
        }
        for (_, ns) in by_port {
            if file_nodes.iter().any(|(known, _)| *known == ns) {
                continue;
            }
            let multiple = ns.len() > 1;
            let id = format!("{}{}", if multiple { "b" } else { "f" }, ns[0]);
            workflow.nodes.push(Node {
                id: id.clone(),
                kind: NodeKind::File {
                    accept: common_mime(&ns, attachments),
                    multiple,
                },
                retries: 0,
                pos: None,
            });
            file_nodes.push((ns, id));
        }
    }
    for call in executed {
        workflow.nodes.push(Node {
            id: call.call_id.clone(),
            kind: NodeKind::Tool {
                tool: call.tool.clone(),
                params: call
                    .params
                    .iter()
                    .map(|(k, v)| (k.clone(), param_text(v)))
                    .collect(),
            },
            retries: 0,
            pos: None,
        });
        let mut linked_files: Vec<&str> = Vec::new();
        for (port, reference) in &call.refs {
            let from = match reference {
                Reference::Text(n) => Endpoint {
                    node: format!("t{n}"),
                    port: TEXT_PORT.to_owned(),
                },
                Reference::Result { call, port } => Endpoint {
                    node: call.clone(),
                    port: port.clone(),
                },
                Reference::File(_) => {
                    // One link per input, whatever the number of files in the batch.
                    if linked_files.contains(&port.as_str()) {
                        continue;
                    }
                    linked_files.push(port);
                    let ns: Vec<usize> = call
                        .refs
                        .iter()
                        .filter_map(|(p, r)| match r {
                            Reference::File(n) if p == port => Some(*n),
                            _ => None,
                        })
                        .collect();
                    let Some((_, id)) = file_nodes.iter().find(|(known, _)| *known == ns) else {
                        continue;
                    };
                    let multiple = ns.len() > 1;
                    Endpoint {
                        node: id.clone(),
                        port: if multiple { "files" } else { "file" }.to_owned(),
                    }
                }
            };
            workflow.edges.push(Edge {
                from,
                to: Endpoint {
                    node: call.call_id.clone(),
                    port: port.clone(),
                },
            });
        }
    }
    workflow
}
