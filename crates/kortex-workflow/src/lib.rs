//! Kortex workflow engine (Flow and Mind): a typed, versioned graph of constants and tools, validated
//! against tool contracts and executed in dependency order. Pure: tool execution is injected
//! through [`Runner`] (installed binaries on desktop), so the same rules run everywhere.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use kortex_contract::PortType;
use kortex_runtime::{CancelToken, FileData, Table, ToolError, Value, ValueMap};
use serde::{Deserialize, Serialize};

pub mod connector;
pub mod http;
pub mod logic;
#[cfg(not(target_arch = "wasm32"))]
pub mod process;

pub use logic::{
    AI_FORMATS, AI_TASKS, AI_TOOL, DELAY_TOOL, FILE_PORT, FILES_PORT, LOGIC_OPS, MAX_DELAY_MS,
    MAX_ITEMS, TESTS, file_ports, logic_ports, parse_options, text_io,
};

/// Version written by this build; older files are migrated, newer ones are refused.
pub const FORMAT_VERSION: u32 = 1;

/// Output port of a constant text node.
pub const TEXT_PORT: &str = "text";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    /// A constant text, available on the `text` port.
    Text { value: String },
    /// A tool of the catalogue; params are kept as text and converted by the contract's types.
    Tool {
        tool: String,
        #[serde(default)]
        params: BTreeMap<String, String>,
    },
    /// A file (or batch) the user picks in the editor. Only its kind is saved: the bytes stay on
    /// the device and are handed to the run, never written into the workflow.
    File {
        /// Mime pattern the node accepts (empty means any).
        #[serde(default)]
        accept: String,
        #[serde(default)]
        multiple: bool,
    },
    /// A built-in text node (condition, filter, merge, delay), see [`logic`].
    Logic {
        op: String,
        #[serde(default)]
        params: BTreeMap<String, String>,
    },
}

/// Ports and params of any node: constants and file nodes by construction, logic nodes from
/// [`logic_ports`], tools from the catalogue. `None` when the tool or operation is unknown.
#[must_use]
pub fn ports_of(kind: &NodeKind, catalogue: &dyn Catalogue) -> Option<ToolPorts> {
    match kind {
        NodeKind::Text { .. } => Some(ToolPorts {
            outputs: vec![Port {
                id: TEXT_PORT.to_owned(),
                ty: PortType::Text,
                required: false,
            }],
            ..ToolPorts::default()
        }),
        NodeKind::Tool { tool, .. } => catalogue.tool(tool).cloned(),
        NodeKind::File { accept, multiple } => Some(file_ports(accept, *multiple)),
        NodeKind::Logic { op, .. } => logic_ports(op),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(flatten)]
    pub kind: NodeKind,
    /// Extra attempts after a failure (0 = run once).
    #[serde(default)]
    pub retries: u8,
    /// Where the editor shows the node (canvas units); absent in files written before the canvas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<Pos>,
}

/// A position on the editor canvas. It never changes what a workflow does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

/// Horizontal and vertical distance between the columns and rows of an automatic layout.
pub const LAYOUT_STEP: (i32, i32) = (288, 170);

/// Gives every node without a position one: columns follow the longest chain that feeds the node,
/// rows follow the order of the nodes inside a column. Positioned nodes are left where they are.
pub fn auto_layout(workflow: &mut Workflow) {
    let mut depth: BTreeMap<String, i32> = BTreeMap::new();
    let ids: Vec<String> = workflow.nodes.iter().map(|n| n.id.clone()).collect();
    // Longest-path layering; bounded passes keep a cyclic file from looping.
    for _ in 0..ids.len() {
        let mut changed = false;
        for edge in &workflow.edges {
            let from = depth.get(&edge.from.node).copied().unwrap_or(0);
            let entry = depth.entry(edge.to.node.clone()).or_insert(0);
            if *entry < from + 1 && (from as usize) < ids.len() {
                *entry = from + 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut rows: BTreeMap<i32, i32> = BTreeMap::new();
    for node in &mut workflow.nodes {
        let column = depth.get(&node.id).copied().unwrap_or(0);
        let row = rows.entry(column).or_insert(0);
        if node.pos.is_none() {
            node.pos = Some(Pos {
                x: 40 + column * LAYOUT_STEP.0,
                y: 40 + *row * LAYOUT_STEP.1,
            });
        }
        *row += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Endpoint {
    pub node: String,
    pub port: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: Endpoint,
    pub to: Endpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workflow {
    pub version: u32,
    pub name: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Default for Workflow {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            name: String::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

/// An input or output port of a tool, from its contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    pub id: String,
    pub ty: PortType,
    pub required: bool,
}

/// A parameter of a tool, from its contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamSpec {
    pub id: String,
    pub ty: PortType,
    pub default: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolPorts {
    pub inputs: Vec<Port>,
    pub params: Vec<ParamSpec>,
    pub outputs: Vec<Port>,
}

impl ToolPorts {
    /// Ports and params as declared by a tool contract.
    #[must_use]
    pub fn from_contract(contract: &kortex_contract::ToolContract) -> Self {
        let default_text = |value: Option<&toml::Value>| match value {
            Some(toml::Value::String(s)) => s.clone(),
            Some(toml::Value::Integer(i)) => i.to_string(),
            Some(toml::Value::Float(f)) => f.to_string(),
            Some(toml::Value::Boolean(b)) => b.to_string(),
            _ => String::new(),
        };
        Self {
            inputs: contract
                .inputs
                .iter()
                .map(|i| Port {
                    id: i.id.clone(),
                    ty: i.ty.clone(),
                    required: i.required,
                })
                .collect(),
            params: contract
                .params
                .iter()
                .map(|p| ParamSpec {
                    id: p.id.clone(),
                    ty: p.ty.clone(),
                    default: default_text(p.default.as_ref()),
                    options: p.options.clone().unwrap_or_default(),
                })
                .collect(),
            outputs: contract
                .outputs
                .iter()
                .map(|o| Port {
                    id: o.id.clone(),
                    ty: o.ty.clone(),
                    required: false,
                })
                .collect(),
        }
    }
}

/// Contracts and ports of every tool of a generated `registry/index.json`.
///
/// # Errors
/// The text is not a registry.
pub fn contracts_from_registry(json: &str) -> Result<Vec<kortex_contract::ToolContract>, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    value["tools"]
        .as_array()
        .ok_or("registre sans `tools`")?
        .iter()
        .map(|t| serde_json::from_value(t.clone()).map_err(|e| e.to_string()))
        .collect()
}

/// Port catalogue for every contract.
#[must_use]
pub fn catalogue_of(contracts: &[kortex_contract::ToolContract]) -> BTreeMap<String, ToolPorts> {
    contracts
        .iter()
        .map(|c| (c.tool.id.clone(), ToolPorts::from_contract(c)))
        .collect()
}

/// What the engine needs to know about tools; hosts build it from the registry.
pub trait Catalogue {
    fn tool(&self, id: &str) -> Option<&ToolPorts>;
}

impl Catalogue for BTreeMap<String, ToolPorts> {
    fn tool(&self, id: &str) -> Option<&ToolPorts> {
        self.get(id)
    }
}

/// A problem found in a workflow; `code` is translated by the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub node: Option<String>,
    pub code: &'static str,
    pub detail: String,
}

fn issue(node: Option<&str>, code: &'static str, detail: impl Into<String>) -> Issue {
    Issue {
        node: node.map(str::to_owned),
        code,
        detail: detail.into(),
    }
}

/// Parses a workflow file, migrating older versions.
///
/// # Errors
/// `invalid_json`, or `too_new_version` for a file written by a newer Flow.
pub fn from_json(text: &str) -> Result<Workflow, Issue> {
    let workflow: Workflow =
        serde_json::from_str(text).map_err(|e| issue(None, "invalid_json", e.to_string()))?;
    if workflow.version > FORMAT_VERSION {
        return Err(issue(None, "too_new_version", workflow.version.to_string()));
    }
    // Version 1 is the first format: nothing to migrate yet.
    Ok(Workflow {
        version: FORMAT_VERSION,
        ..workflow
    })
}

#[must_use]
pub fn to_json(workflow: &Workflow) -> String {
    serde_json::to_string_pretty(workflow).unwrap_or_default()
}

fn output_port(
    workflow: &Workflow,
    catalogue: &dyn Catalogue,
    node: &str,
    port: &str,
) -> Result<PortType, &'static str> {
    let found = workflow
        .nodes
        .iter()
        .find(|n| n.id == node)
        .ok_or("unknown_node")?;
    let ports = ports_of(&found.kind, catalogue).ok_or(match found.kind {
        NodeKind::Tool { .. } => "unknown_tool",
        _ => "unknown_logic",
    })?;
    ports
        .outputs
        .iter()
        .find(|p| p.id == port)
        .map(|p| p.ty.clone())
        .ok_or("unknown_port")
}

/// A URL with every `{secret:NAME}` replaced by a plain word, to check its shape and host.
fn url_for_check(url: &str) -> String {
    http::substitute(url, &|_| Some("secret".to_owned())).unwrap_or_else(|_| url.to_owned())
}

/// Everything wrong with `workflow`, in a stable order; empty when it can run.
#[must_use]
pub fn validate(workflow: &Workflow, catalogue: &dyn Catalogue) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    for node in &workflow.nodes {
        if !seen.insert(node.id.as_str()) {
            issues.push(issue(Some(&node.id), "duplicate_node", node.id.clone()));
        }
        let named = match &node.kind {
            NodeKind::Tool { tool, params } => Some((tool.as_str(), params)),
            NodeKind::Logic { op, params } => Some((op.as_str(), params)),
            _ => None,
        };
        if let Some((name, params)) = named {
            match ports_of(&node.kind, catalogue) {
                None => issues.push(issue(
                    Some(&node.id),
                    if matches!(node.kind, NodeKind::Tool { .. }) {
                        "unknown_tool"
                    } else {
                        "unknown_logic"
                    },
                    name.to_owned(),
                )),
                Some(ports) => {
                    if let Err(e) = param_values(params, &ports.params) {
                        issues.push(Issue {
                            node: Some(node.id.clone()),
                            ..e
                        });
                    } else if name == "http" && matches!(node.kind, NodeKind::Logic { .. }) {
                        // Checked with placeholders kept: a secret may stand in the path or a header.
                        let wired_url = workflow
                            .edges
                            .iter()
                            .any(|e| e.to.node == node.id && e.to.port == "url");
                        let url = params.get("url").map_or("", String::as_str).trim();
                        let url_ok = wired_url || http::check_url(&url_for_check(url)).is_ok();
                        let headers_ok = http::parse_headers(
                            &http::substitute(
                                params.get("headers").map_or("", String::as_str),
                                &|_| Some("secret".to_owned()),
                            )
                            .unwrap_or_default(),
                        )
                        .is_ok();
                        if !url_ok {
                            issues.push(issue(Some(&node.id), "bad_param", "url"));
                        }
                        if !headers_ok {
                            issues.push(issue(Some(&node.id), "bad_param", "headers"));
                        }
                    } else if name == "ai" && matches!(node.kind, NodeKind::Logic { .. }) {
                        // Every tool the agent may call must exist and work on one text.
                        let listed = params.get("tools").map_or("", String::as_str);
                        let bad = listed
                            .split(',')
                            .map(str::trim)
                            .filter(|t| !t.is_empty())
                            .any(|t| {
                                !catalogue
                                    .tool(t)
                                    .is_some_and(|p| logic::text_io(p).is_some())
                            });
                        if bad {
                            issues.push(issue(Some(&node.id), "bad_param", "tools"));
                        }
                    } else if name == "foreach" && matches!(node.kind, NodeKind::Logic { .. }) {
                        let chosen = params.get("tool").map_or("", String::as_str);
                        let usable = catalogue
                            .tool(chosen)
                            .is_some_and(|t| logic::text_io(t).is_some());
                        if !usable {
                            issues.push(issue(Some(&node.id), "bad_param", "tool"));
                        }
                    }
                }
            }
        }
    }
    let mut taken: BTreeSet<&Endpoint> = BTreeSet::new();
    for edge in &workflow.edges {
        let source = output_port(workflow, catalogue, &edge.from.node, &edge.from.port);
        let target = workflow.nodes.iter().find(|n| n.id == edge.to.node);
        let target_ty = target
            .and_then(|n| ports_of(&n.kind, catalogue))
            .and_then(|p| p.inputs.into_iter().find(|i| i.id == edge.to.port))
            .map(|p| p.ty);
        match (source, target_ty) {
            (Err(code), _) => {
                issues.push(issue(Some(&edge.from.node), code, edge.from.port.clone()))
            }
            (Ok(_), None) => issues.push(issue(
                Some(&edge.to.node),
                if target.is_none() {
                    "unknown_node"
                } else {
                    "unknown_port"
                },
                edge.to.port.clone(),
            )),
            (Ok(from), Some(to)) => {
                if !from.converts_to(&to) {
                    issues.push(issue(
                        Some(&edge.to.node),
                        "type_mismatch",
                        format!("{from} → {to}"),
                    ));
                }
                if !taken.insert(&edge.to) {
                    issues.push(issue(
                        Some(&edge.to.node),
                        "input_taken",
                        edge.to.port.clone(),
                    ));
                }
            }
        }
    }
    for node in &workflow.nodes {
        let Some(ports) = ports_of(&node.kind, catalogue) else {
            continue;
        };
        for input in ports.inputs.iter().filter(|i| i.required) {
            let wired = workflow
                .edges
                .iter()
                .any(|e| e.to.node == node.id && e.to.port == input.id);
            if !wired {
                issues.push(issue(Some(&node.id), "missing_input", input.id.clone()));
            }
        }
    }
    if order(workflow).is_err() {
        issues.push(issue(None, "cycle", String::new()));
    }
    issues
}

/// The graph contains a cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cycle;

/// Execution order (Kahn): every node after the nodes it reads from.
///
/// # Errors
/// [`Cycle`] when the graph cannot be ordered.
pub fn order(workflow: &Workflow) -> Result<Vec<usize>, Cycle> {
    let index: BTreeMap<&str, usize> = workflow
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut incoming = vec![0_usize; workflow.nodes.len()];
    let mut next: Vec<Vec<usize>> = vec![Vec::new(); workflow.nodes.len()];
    for edge in &workflow.edges {
        if let (Some(&from), Some(&to)) = (
            index.get(edge.from.node.as_str()),
            index.get(edge.to.node.as_str()),
        ) {
            incoming[to] += 1;
            next[from].push(to);
        }
    }
    let mut ready: VecDeque<usize> = (0..incoming.len()).filter(|i| incoming[*i] == 0).collect();
    let mut sorted = Vec::with_capacity(incoming.len());
    while let Some(node) = ready.pop_front() {
        sorted.push(node);
        for &to in &next[node] {
            incoming[to] -= 1;
            if incoming[to] == 0 {
                ready.push_back(to);
            }
        }
    }
    if sorted.len() == workflow.nodes.len() {
        Ok(sorted)
    } else {
        Err(Cycle)
    }
}

/// Converts text params to typed values using the contract's param types; missing keys use the
/// contract default.
///
/// # Errors
/// `bad_param` naming the parameter whose text is not a valid value.
pub fn param_values(
    params: &BTreeMap<String, String>,
    specs: &[ParamSpec],
) -> Result<ValueMap, Issue> {
    let mut out = ValueMap::new();
    for spec in specs {
        let text = params.get(&spec.id).unwrap_or(&spec.default);
        let bad = || issue(None, "bad_param", spec.id.clone());
        let value = match spec.ty {
            PortType::Integer => Value::Int(text.trim().parse().map_err(|_| bad())?),
            PortType::Number => Value::Number(text.trim().parse().map_err(|_| bad())?),
            PortType::Bool => Value::Bool(match text.trim() {
                "true" => true,
                "false" => false,
                _ => return Err(bad()),
            }),
            PortType::Enum => {
                if !spec.options.is_empty() && !spec.options.contains(text) {
                    return Err(bad());
                }
                Value::Text(text.clone())
            }
            _ => Value::Text(text.clone()),
        };
        out.insert(spec.id.clone(), value);
    }
    Ok(out)
}

fn csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

fn csv(table: &Table) -> String {
    let line = |cells: &[String]| {
        cells
            .iter()
            .map(|c| csv_field(c))
            .collect::<Vec<_>>()
            .join(",")
    };
    std::iter::once(line(&table.columns))
        .chain(table.rows.iter().map(|r| line(r)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Safe adapter between ports (the transformations behind `PortType::converts_to`).
///
/// # Errors
/// `invalid_param` when the value cannot be adapted to the target type.
pub fn adapt(value: Value, to: &PortType) -> Result<Value, ToolError> {
    Ok(match (value, to) {
        (Value::Text(text), PortType::File(_)) => Value::File(FileData {
            name: "text.txt".into(),
            mime: "text/plain".into(),
            bytes: text.into_bytes(),
        }),
        (Value::Table(table), PortType::File(_)) => Value::File(FileData {
            name: "table.csv".into(),
            mime: "text/csv".into(),
            bytes: csv(&table).into_bytes(),
        }),
        (Value::File(file), PortType::Files(_)) => Value::Files(vec![file]),
        // Identical types (and any pair `converts_to` accepts without reshaping) pass as is.
        (value, _) => value,
    })
}

/// A tool run that failed. The code is a string because an external engine reports its own
/// codes (translated by the tool's locales), not only the ones compiled into this program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunError {
    pub code: String,
    pub detail: Option<String>,
}

impl From<ToolError> for RunError {
    fn from(error: ToolError) -> Self {
        Self {
            code: error.code.to_owned(),
            detail: error.detail,
        }
    }
}

/// Executes one tool: the host decides how (installed binary, in-process engine, test double).
pub trait Runner {
    /// # Errors
    /// The tool's own typed error, or an infrastructure error with a `worker_*` code.
    fn run(&self, tool: &str, inputs: ValueMap, params: ValueMap) -> Result<ValueMap, RunError>;
}

/// Outputs of every node that ran, plus a short journal (no file contents).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub results: BTreeMap<String, ValueMap>,
    pub journal: Vec<String>,
    /// Nodes that did not run because a branch they depend on was not taken.
    pub skipped: Vec<String>,
}

/// A failed run: where it stopped and what already ran.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub node: Option<String>,
    pub code: String,
    pub detail: Option<String>,
    pub report: Report,
}

fn fail(node: Option<&str>, code: &str, detail: Option<String>, report: Report) -> Failure {
    Failure {
        node: node.map(str::to_owned),
        code: code.to_owned(),
        detail,
        report,
    }
}

/// One tool run requested by an [`Execution`].
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub node: String,
    pub tool: String,
    pub inputs: ValueMap,
    pub params: ValueMap,
}

/// What an [`Execution`] wants next.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Run this job, then report its result with [`Execution::finish`].
    Run(Job),
    Done(Report),
    Failed(Failure),
}

/// A workflow run driven one tool at a time. The caller decides how a job runs (blocking on
/// desktop, a Web Worker on the web, a test double) and reports back; ordering, data flow,
/// adapters, retries, cancellation and the journal live here, once.
pub struct Execution<'a> {
    workflow: &'a Workflow,
    catalogue: &'a dyn Catalogue,
    sequence: Vec<usize>,
    cursor: usize,
    attempt: u8,
    pending: Option<Job>,
    failure: Option<(String, RunError)>,
    report: Report,
    provided: BTreeMap<String, Value>,
    batch: Option<Batch>,
}

/// A "for each line" node in progress: one job per line, results joined at the end.
struct Batch {
    node: String,
    tool: String,
    port: String,
    params: ValueMap,
    lines: VecDeque<String>,
    done: Vec<String>,
    output: String,
}

impl<'a> Execution<'a> {
    /// The bytes chosen for each file node (by node id). They are never part of the workflow.
    #[must_use]
    pub fn with_files(mut self, provided: BTreeMap<String, Value>) -> Self {
        self.provided = provided;
        self
    }

    /// # Errors
    /// `invalid_workflow` (naming the first problem) when the workflow cannot run.
    // `Failure` carries the partial report; it is built once per run, never in a hot path.
    #[allow(clippy::result_large_err)]
    pub fn new(workflow: &'a Workflow, catalogue: &'a dyn Catalogue) -> Result<Self, Failure> {
        if let Some(first) = validate(workflow, catalogue).first() {
            return Err(fail(
                first.node.as_deref(),
                "invalid_workflow",
                Some(first.code.to_owned()),
                Report::default(),
            ));
        }
        let sequence = order(workflow).map_err(|Cycle| {
            fail(
                None,
                "invalid_workflow",
                Some("cycle".into()),
                Report::default(),
            )
        })?;
        Ok(Self {
            workflow,
            catalogue,
            sequence,
            cursor: 0,
            attempt: 0,
            pending: None,
            failure: None,
            report: Report::default(),
            provided: BTreeMap::new(),
            batch: None,
        })
    }

    fn failed(&self, node: Option<&str>, code: &str, detail: Option<String>) -> Step {
        Step::Failed(fail(node, code, detail, self.report.clone()))
    }

    fn next_batch_job(&mut self) -> Option<Job> {
        let batch = self.batch.as_mut()?;
        let line = batch.lines.pop_front()?;
        Some(Job {
            node: batch.node.clone(),
            tool: batch.tool.clone(),
            inputs: ValueMap::from([(batch.port.clone(), Value::Text(line))]),
            params: batch.params.clone(),
        })
    }

    fn finish_batch(&mut self) {
        if let Some(batch) = self.batch.take() {
            self.report.journal.push(format!("{}: ok", batch.node));
            self.report.results.insert(
                batch.node,
                ValueMap::from([(TEXT_PORT.to_owned(), Value::Text(batch.done.join("\n")))]),
            );
            self.cursor += 1;
        }
    }

    /// The next job, or the end of the run. Constant nodes are resolved here without a job.
    pub fn next(&mut self, cancel: &CancelToken) -> Step {
        if let Some((node, error)) = self.failure.take() {
            return self.failed(Some(&node), &error.code, error.detail);
        }
        if let Some(job) = &self.pending {
            // A loop runs many jobs for one node: cancellation is checked between them.
            if self.batch.is_some() && cancel.is_cancelled() {
                let node = job.node.clone();
                return self.failed(Some(&node), ToolError::CANCELLED, None);
            }
            return Step::Run(job.clone());
        }
        while let Some(&position) = self.sequence.get(self.cursor) {
            let node = &self.workflow.nodes[position];
            if cancel.is_cancelled() {
                return self.failed(Some(&node.id), ToolError::CANCELLED, None);
            }
            let (name, params) = match &node.kind {
                NodeKind::Text { value } => {
                    self.report.journal.push(format!("{}: ok", node.id));
                    self.report.results.insert(
                        node.id.clone(),
                        ValueMap::from([(TEXT_PORT.to_owned(), Value::Text(value.clone()))]),
                    );
                    self.cursor += 1;
                    continue;
                }
                NodeKind::File { accept, multiple } => {
                    let Some(value) = self.provided.get(&node.id).cloned() else {
                        return self.failed(Some(&node.id), "missing_file", None);
                    };
                    let port = file_ports(accept, *multiple).outputs.remove(0).id;
                    self.report.journal.push(format!("{}: ok", node.id));
                    self.report
                        .results
                        .insert(node.id.clone(), ValueMap::from([(port, value)]));
                    self.cursor += 1;
                    continue;
                }
                NodeKind::Tool { tool, params } => (tool.clone(), params),
                NodeKind::Logic { op, params } => (op.clone(), params),
            };
            let is_logic = matches!(node.kind, NodeKind::Logic { .. });
            let ports = ports_of(&node.kind, self.catalogue).unwrap_or_default();
            let mut inputs = ValueMap::new();
            let mut unavailable = Vec::new();
            for edge in self.workflow.edges.iter().filter(|e| e.to.node == node.id) {
                let produced = self
                    .report
                    .results
                    .get(&edge.from.node)
                    .and_then(|r| r.get(&edge.from.port))
                    .cloned();
                let Some(produced) = produced else {
                    // The branch was not taken (or its node was skipped): not a failure.
                    let branch = self.report.results.contains_key(&edge.from.node)
                        || self.report.skipped.contains(&edge.from.node);
                    if branch {
                        unavailable.push(edge.to.port.clone());
                        continue;
                    }
                    return self.failed(
                        Some(&node.id),
                        "missing_input",
                        Some(edge.to.port.clone()),
                    );
                };
                let value = match ports.inputs.iter().find(|p| p.id == edge.to.port) {
                    Some(port) => match adapt(produced, &port.ty) {
                        Ok(value) => value,
                        Err(e) => return self.failed(Some(&node.id), e.code, e.detail),
                    },
                    None => produced,
                };
                inputs.insert(edge.to.port.clone(), value);
            }
            let skip = if is_logic && (name == "http" || name == "ai") {
                // A request never goes out with a missing piece: a skipped feeder skips it.
                !unavailable.is_empty()
            } else if is_logic && (name == "merge" || name == "template") {
                inputs.is_empty() && (name == "merge" || !unavailable.is_empty())
            } else {
                ports
                    .inputs
                    .iter()
                    .any(|p| p.required && unavailable.contains(&p.id))
            };
            if skip {
                self.report.journal.push(format!("{}: skipped", node.id));
                self.report.skipped.push(node.id.clone());
                self.cursor += 1;
                continue;
            }
            let params = match param_values(params, &ports.params) {
                Ok(params) => params,
                Err(e) => return self.failed(Some(&node.id), e.code, Some(e.detail)),
            };
            if is_logic && name == "foreach" {
                let text = match inputs.get("text") {
                    Some(Value::Text(t)) => t.clone(),
                    _ => return self.failed(Some(&node.id), "missing_input", Some("text".into())),
                };
                let tool = match params.get("tool") {
                    Some(Value::Text(t)) => t.clone(),
                    _ => String::new(),
                };
                let max = match params.get("max") {
                    Some(Value::Int(m)) => usize::try_from((*m).clamp(1, MAX_ITEMS)).unwrap_or(1),
                    _ => 100,
                };
                let spec = self.catalogue.tool(&tool).cloned().unwrap_or_default();
                let Some((input, output)) = logic::text_io(&spec) else {
                    return self.failed(Some(&node.id), "bad_param", Some("tool".into()));
                };
                let options = match &node.kind {
                    NodeKind::Logic { params, .. } => {
                        logic::parse_options(params.get("options").map_or("", String::as_str))
                    }
                    _ => BTreeMap::new(),
                };
                let inner = match param_values(&options, &spec.params) {
                    Ok(inner) => inner,
                    Err(e) => return self.failed(Some(&node.id), e.code, Some(e.detail)),
                };
                let lines: VecDeque<String> = text.lines().map(str::to_owned).collect();
                if lines.len() > max {
                    return self.failed(Some(&node.id), "too_many_items", Some(max.to_string()));
                }
                self.batch = Some(Batch {
                    node: node.id.clone(),
                    tool,
                    port: input,
                    params: inner,
                    lines,
                    done: Vec::new(),
                    output,
                });
                if let Some(job) = self.next_batch_job() {
                    self.attempt = 0;
                    self.pending = Some(job.clone());
                    return Step::Run(job);
                }
                // No lines: nothing to run, the result is empty.
                self.finish_batch();
                continue;
            }
            if is_logic && name != "delay" && name != "http" && name != "ai" {
                match logic::eval(&name, &inputs, &params) {
                    Ok(outputs) => {
                        self.report.journal.push(format!("{}: ok", node.id));
                        self.report.results.insert(node.id.clone(), outputs);
                        self.cursor += 1;
                        continue;
                    }
                    Err(e) => return self.failed(Some(&node.id), &e.code, e.detail),
                }
            }
            let tool = if !is_logic {
                name
            } else if name == "http" {
                http::HTTP_TOOL.to_owned()
            } else if name == "ai" {
                AI_TOOL.to_owned()
            } else {
                DELAY_TOOL.to_owned()
            };
            let job = Job {
                node: node.id.clone(),
                tool: tool.clone(),
                inputs,
                params,
            };
            self.attempt = 0;
            self.pending = Some(job.clone());
            return Step::Run(job);
        }
        Step::Done(self.report.clone())
    }

    /// Reports the outcome of the job last returned by [`Self::next`]: success moves on, a failure
    /// is retried while the node allows it (never for a cancellation), then ends the run.
    pub fn finish(&mut self, result: Result<ValueMap, RunError>) {
        let Some(job) = self.pending.clone() else {
            return;
        };
        match result {
            Ok(outputs) if self.batch.is_some() => {
                let produced = self
                    .batch
                    .as_ref()
                    .and_then(|b| match outputs.get(&b.output) {
                        Some(Value::Text(t)) => Some(t.clone()),
                        _ => None,
                    });
                if let Some(batch) = &mut self.batch {
                    batch.done.push(produced.unwrap_or_default());
                }
                self.attempt = 0;
                if let Some(next) = self.next_batch_job() {
                    self.pending = Some(next);
                } else {
                    self.pending = None;
                    self.finish_batch();
                }
            }
            Ok(outputs) => {
                self.pending = None;
                self.cursor += 1;
                self.report.journal.push(format!("{}: ok", job.node));
                self.report.results.insert(job.node, outputs);
            }
            Err(error) => {
                let retries = self
                    .workflow
                    .nodes
                    .iter()
                    .find(|n| n.id == job.node)
                    .map_or(0, |n| n.retries);
                if self.attempt < retries && error.code != ToolError::CANCELLED {
                    self.attempt += 1;
                    self.report.journal.push(format!(
                        "{}: retry {} ({})",
                        job.node, self.attempt, error.code
                    ));
                } else {
                    self.pending = None;
                    self.failure = Some((job.node, error));
                }
            }
        }
    }
}

/// Runs the workflow to the end on the calling thread, blocking on every tool. Stops at the first
/// failing node (after its retries), checks `cancel` between nodes and never calls a runner for an
/// invalid workflow.
///
/// # Errors
/// [`Failure`] with `invalid_workflow`, `cancelled`, or the failing node's error code.
#[allow(clippy::result_large_err)]
pub fn run(
    workflow: &Workflow,
    catalogue: &dyn Catalogue,
    runner: &dyn Runner,
    cancel: &CancelToken,
) -> Result<Report, Failure> {
    let mut execution = Execution::new(workflow, catalogue)?;
    loop {
        match execution.next(cancel) {
            Step::Run(job) => execution.finish(runner.run(&job.tool, job.inputs, job.params)),
            Step::Done(report) => return Ok(report),
            Step::Failed(failure) => return Err(failure),
        }
    }
}
