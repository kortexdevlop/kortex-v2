//! Tool ports and params of the whole catalogue, read from the registry contracts.
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock, RwLock};

use kortex_shell as shell;

use crate::engine::{Catalogue, NodeKind, ParamSpec, Port, ToolPorts, Workflow};

fn default_text(value: Option<&toml::Value>) -> String {
    match value {
        Some(toml::Value::String(s)) => s.clone(),
        Some(toml::Value::Integer(i)) => i.to_string(),
        Some(toml::Value::Float(f)) => f.to_string(),
        Some(toml::Value::Boolean(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// The tools a workflow can use: the catalogue's tools and the connector actions, plus the
/// workflows the user saved that have inputs or outputs (they come and go while the app runs).
/// The catalogue part follows the catalogue in use: a newer verified catalogue (ADR 0034) adds its
/// tools without rebuilding Flow.
pub struct Catalog {
    base: RwLock<&'static BTreeMap<String, ToolPorts>>,
    seen: Mutex<Option<shell::Catalogue>>,
    extra: RwLock<BTreeMap<String, &'static ToolPorts>>,
}

impl Catalog {
    fn base(&self) -> &'static BTreeMap<String, ToolPorts> {
        *self
            .base
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Rebuilds the fixed tools when the catalogue in use changed (once per new catalogue; the
    /// previous map is leaked on purpose so references handed out earlier stay valid).
    fn sync(&self) {
        let current = shell::catalogue();
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if seen.as_ref().is_some_and(|s| s.same(&current)) {
            return;
        }
        let map: &'static BTreeMap<String, ToolPorts> = Box::leak(Box::new(build(&current)));
        *self
            .base
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = map;
        *seen = Some(current);
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ToolPorts> {
        self.base()
            .get(id)
            .or_else(|| self.extra.read().ok().and_then(|e| e.get(id).copied()))
    }

    #[must_use]
    pub fn contains_key(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    /// The fixed tools (registry and connectors), not the saved workflows.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ToolPorts)> {
        self.base().iter()
    }
}

impl Catalogue for Catalog {
    fn tool(&self, id: &str) -> Option<&ToolPorts> {
        self.get(id)
    }
}

/// Replaces the saved workflows offered as tools. An unchanged one is kept as is, so repeated
/// calls (every render) allocate nothing.
pub fn set_workflow_tools(tools: Vec<(String, String, ToolPorts)>) {
    let catalog = ports();
    if let Ok(mut names) = workflow_names().write() {
        *names = tools
            .iter()
            .map(|(id, name, _)| (id.clone(), name.clone()))
            .collect();
    }
    let tools: BTreeMap<String, ToolPorts> = tools.into_iter().map(|(id, _, p)| (id, p)).collect();
    let Ok(mut extra) = catalog.extra.write() else {
        return;
    };
    extra.retain(|id, _| tools.contains_key(id));
    for (id, ports) in tools {
        if extra.get(&id).is_none_or(|known| **known != ports) {
            // Small and rare (only when a saved workflow's doors change): leaked on purpose so a
            // reference can outlive the lock.
            extra.insert(id, Box::leak(Box::new(ports)));
        }
    }
}

fn workflow_names() -> &'static RwLock<BTreeMap<String, String>> {
    static NAMES: OnceLock<RwLock<BTreeMap<String, String>>> = OnceLock::new();
    NAMES.get_or_init(|| RwLock::new(BTreeMap::new()))
}

/// Display name of a saved workflow used as a tool (`workflow.<id>`).
#[must_use]
pub fn workflow_name(tool: &str) -> Option<String> {
    workflow_names()
        .read()
        .ok()
        .and_then(|n| n.get(tool).cloned())
}

fn build(catalogue: &shell::Catalogue) -> BTreeMap<String, ToolPorts> {
    {
        let mut all: BTreeMap<String, ToolPorts> = catalogue
            .iter()
            .map(|entry| {
                let ports = ToolPorts {
                    inputs: entry
                        .inputs
                        .iter()
                        .map(|i| Port {
                            id: i.id.clone(),
                            ty: i.ty.clone(),
                            required: i.required,
                        })
                        .collect(),
                    params: entry
                        .params
                        .iter()
                        .map(|p| ParamSpec {
                            id: p.id.clone(),
                            ty: p.ty.clone(),
                            default: default_text(p.default.as_ref()),
                            options: p.options.clone().unwrap_or_default(),
                        })
                        .collect(),
                    outputs: entry
                        .outputs
                        .iter()
                        .map(|o| Port {
                            id: o.id.clone(),
                            ty: o.ty.clone(),
                            required: false,
                        })
                        .collect(),
                };
                (entry.tool.id.clone(), ports)
            })
            .collect();
        // Connector actions are tools too: same ports, same params, run by the host.
        for connector in crate::connectors::all() {
            for action in &connector.actions {
                all.insert(connector.tool_id(&action.id), action.ports());
            }
        }
        all
    }
}

/// Tool ports and params for the whole catalogue in use, plus connectors and saved workflows.
pub fn ports() -> &'static Catalog {
    static PORTS: OnceLock<Catalog> = OnceLock::new();
    static EMPTY: OnceLock<BTreeMap<String, ToolPorts>> = OnceLock::new();
    let catalog = PORTS.get_or_init(|| Catalog {
        base: RwLock::new(EMPTY.get_or_init(BTreeMap::new)),
        seen: Mutex::new(None),
        extra: RwLock::new(BTreeMap::new()),
    });
    catalog.sync();
    catalog
}

/// Every tool of the catalogue by display name: file inputs are fed by file nodes, text inputs by
/// text, logic or other tools.
pub fn usable_tools(lang: &str) -> Vec<(String, String)> {
    let mut tools: Vec<(String, String)> = shell::catalogue()
        .iter()
        .map(|e| (e.tool.id.clone(), shell::name(e, lang)))
        .collect();
    tools.sort_by(|a, b| a.1.cmp(&b.1));
    tools
}

/// Connector actions for the palette: tool id and "Connector — action".
pub fn connector_tools(lang: &str) -> Vec<(String, String)> {
    crate::connectors::all()
        .iter()
        .flat_map(|c| c.actions.iter().map(|a| c.tool_id(&a.id)))
        .filter_map(|id| crate::connectors::display_name(&id, lang).map(|name| (id, name)))
        .collect()
}

/// A port name usable as an identifier: lowercase letters, digits, `_` and `-`.
fn port_name(raw: &str, fallback: &str, taken: &[String]) -> String {
    let cleaned: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let base = match cleaned.trim_matches('_') {
        "" => fallback.to_owned(),
        trimmed => trimmed.to_owned(),
    };
    let mut name = base.clone();
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{base}_{n}");
        n += 1;
    }
    name
}

/// `(port name, node id)` pairs.
pub type Doors = Vec<(String, String)>;

/// The doors of a workflow: `(port name, node id)` of its input cards and of its output cards.
#[must_use]
pub fn doors(workflow: &Workflow) -> (Doors, Doors) {
    let collect = |wanted: &str, fallback: &str| {
        let mut out: Vec<(String, String)> = Vec::new();
        for node in &workflow.nodes {
            if let NodeKind::Logic { op, params } = &node.kind
                && op == wanted
            {
                let taken: Vec<String> = out.iter().map(|(p, _)| p.clone()).collect();
                let name = port_name(
                    params.get("name").map_or("", String::as_str),
                    fallback,
                    &taken,
                );
                out.push((name, node.id.clone()));
            }
        }
        out
    };
    (collect("input", "input"), collect("output", "output"))
}

/// A saved workflow as a tool: one text input per input card, one text output per output card.
/// `None` when it has neither.
#[must_use]
pub fn workflow_ports(workflow: &Workflow) -> Option<ToolPorts> {
    let (inputs, outputs) = doors(workflow);
    if inputs.is_empty() && outputs.is_empty() {
        return None;
    }
    let port = |id: &str| Port {
        id: id.to_owned(),
        ty: kortex_contract::PortType::Text,
        required: false,
    };
    Some(ToolPorts {
        inputs: inputs.iter().map(|(p, _)| port(p)).collect(),
        params: Vec::new(),
        outputs: outputs.iter().map(|(p, _)| port(p)).collect(),
    })
}

/// Tool id of a saved workflow.
#[must_use]
pub fn workflow_tool_id(saved_id: &str) -> String {
    format!("workflow.{saved_id}")
}

/// The saved workflows usable as cards (not the one being edited, not deleted ones): tool id, name
/// and ports.
#[must_use]
pub fn workflow_tools(
    library: &crate::library::Library,
    editing: Option<&str>,
) -> Vec<(String, String, ToolPorts)> {
    library
        .visible()
        .into_iter()
        .filter(|s| Some(s.id.as_str()) != editing)
        .filter_map(|s| {
            workflow_ports(&s.workflow).map(|p| (workflow_tool_id(&s.id), s.name.clone(), p))
        })
        .collect()
}
