//! Tool ports and params of the whole catalogue, read from the registry contracts.
use std::collections::BTreeMap;
use std::sync::OnceLock;

use kortex_shell as shell;

use crate::engine::{ParamSpec, Port, ToolPorts};

fn default_text(value: Option<&toml::Value>) -> String {
    match value {
        Some(toml::Value::String(s)) => s.clone(),
        Some(toml::Value::Integer(i)) => i.to_string(),
        Some(toml::Value::Float(f)) => f.to_string(),
        Some(toml::Value::Boolean(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// Tool ports and params for the whole catalogue, from the registry contracts.
pub fn ports() -> &'static BTreeMap<String, ToolPorts> {
    static PORTS: OnceLock<BTreeMap<String, ToolPorts>> = OnceLock::new();
    PORTS.get_or_init(|| {
        let mut all: BTreeMap<String, ToolPorts> = shell::catalogue()
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
    })
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
