//! Kortex workflow engine (Flow and Mind): a typed, versioned graph of constants and tools, validated
//! against tool contracts and executed in dependency order. Pure: tool execution is injected
//! through [`Runner`] (installed binaries on desktop), so the same rules run everywhere.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use kortex_contract::PortType;
use kortex_runtime::{CancelToken, FileData, Table, ToolError, Value, ValueMap};
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
pub mod process;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(flatten)]
    pub kind: NodeKind,
    /// Extra attempts after a failure (0 = run once).
    #[serde(default)]
    pub retries: u8,
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
    match &found.kind {
        NodeKind::Text { .. } => (port == TEXT_PORT)
            .then_some(PortType::Text)
            .ok_or("unknown_port"),
        NodeKind::Tool { tool, .. } => catalogue
            .tool(tool)
            .ok_or("unknown_tool")?
            .outputs
            .iter()
            .find(|p| p.id == port)
            .map(|p| p.ty.clone())
            .ok_or("unknown_port"),
    }
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
        if let NodeKind::Tool { tool, params } = &node.kind {
            match catalogue.tool(tool) {
                None => issues.push(issue(Some(&node.id), "unknown_tool", tool.clone())),
                Some(ports) => {
                    if let Err(e) = param_values(params, &ports.params) {
                        issues.push(Issue {
                            node: Some(node.id.clone()),
                            ..e
                        });
                    }
                }
            }
        }
    }
    let mut taken: BTreeSet<&Endpoint> = BTreeSet::new();
    for edge in &workflow.edges {
        let source = output_port(workflow, catalogue, &edge.from.node, &edge.from.port);
        let target = workflow.nodes.iter().find(|n| n.id == edge.to.node);
        let target_ty = match target.map(|n| &n.kind) {
            Some(NodeKind::Tool { tool, .. }) => catalogue
                .tool(tool)
                .and_then(|p| p.inputs.iter().find(|i| i.id == edge.to.port))
                .map(|p| p.ty.clone()),
            _ => None,
        };
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
        let NodeKind::Tool { tool, .. } = &node.kind else {
            continue;
        };
        let Some(ports) = catalogue.tool(tool) else {
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
}

impl<'a> Execution<'a> {
    /// # Errors
    /// `invalid_workflow` (naming the first problem) when the workflow cannot run.
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
        })
    }

    fn failed(&self, node: Option<&str>, code: &str, detail: Option<String>) -> Step {
        Step::Failed(fail(node, code, detail, self.report.clone()))
    }

    /// The next job, or the end of the run. Constant nodes are resolved here without a job.
    pub fn next(&mut self, cancel: &CancelToken) -> Step {
        if let Some((node, error)) = self.failure.take() {
            return self.failed(Some(&node), &error.code, error.detail);
        }
        if let Some(job) = &self.pending {
            return Step::Run(job.clone());
        }
        while let Some(&position) = self.sequence.get(self.cursor) {
            let node = &self.workflow.nodes[position];
            if cancel.is_cancelled() {
                return self.failed(Some(&node.id), ToolError::CANCELLED, None);
            }
            let NodeKind::Tool { tool, params } = &node.kind else {
                if let NodeKind::Text { value } = &node.kind {
                    self.report.journal.push(format!("{}: ok", node.id));
                    self.report.results.insert(
                        node.id.clone(),
                        ValueMap::from([(TEXT_PORT.to_owned(), Value::Text(value.clone()))]),
                    );
                }
                self.cursor += 1;
                continue;
            };
            let ports = self.catalogue.tool(tool).cloned().unwrap_or_default();
            let mut inputs = ValueMap::new();
            for edge in self.workflow.edges.iter().filter(|e| e.to.node == node.id) {
                let Some(produced) = self
                    .report
                    .results
                    .get(&edge.from.node)
                    .and_then(|r| r.get(&edge.from.port))
                    .cloned()
                else {
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
            let params = match param_values(params, &ports.params) {
                Ok(params) => params,
                Err(e) => return self.failed(Some(&node.id), e.code, Some(e.detail)),
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
