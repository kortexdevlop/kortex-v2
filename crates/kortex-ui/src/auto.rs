use std::collections::BTreeMap;

use dioxus::prelude::*;
use kortex_contract::{Localized, PortType, ToolContract};

use crate::components::{Button, Card, DropZone, ModeSwitch, Switch, TextField};
use crate::{Mode, visible_params};

/// Current param values keyed by param id (stringly typed at the UI boundary).
pub type ParamValues = BTreeMap<String, String>;

fn tr<'a>(l: &'a Localized, lang: &str, fallback: &'a str) -> &'a str {
    l.get(lang)
        .or_else(|| l.get("en"))
        .map_or(fallback, String::as_str)
}

fn default_text(v: Option<&toml::Value>) -> String {
    match v {
        Some(toml::Value::String(s)) => s.clone(),
        Some(toml::Value::Integer(i)) => i.to_string(),
        Some(toml::Value::Float(f)) => f.to_string(),
        Some(toml::Value::Boolean(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// UI generated entirely from a tool contract: drop zone / input, params by mode, run button.
#[component]
pub fn AutoTool(
    contract: ToolContract,
    lang: String,
    on_run: EventHandler<ParamValues>,
) -> Element {
    let mut mode = use_signal(Mode::default);
    let mut values = use_signal(|| {
        contract
            .params
            .iter()
            .map(|p| (p.id.clone(), default_text(p.default.as_ref())))
            .collect::<ParamValues>()
    });
    let has_file_input = contract
        .inputs
        .iter()
        .any(|i| matches!(i.ty, PortType::File(_) | PortType::Files(_)));
    let name = tr(&contract.tool.name, &lang, &contract.tool.id).to_owned();
    let shown: Vec<_> = visible_params(&contract, mode())
        .into_iter()
        .cloned()
        .collect();

    rsx! {
        Card {
            header { class: "k-tool-head",
                h2 { "{name}" }
                ModeSwitch {
                    mode: mode(),
                    onchange: move |m| mode.set(m),
                    labels: ["Simple".to_owned(), "Medium".to_owned(), "Pro".to_owned()],
                }
            }
            if has_file_input {
                DropZone { label: "Déposer un fichier".to_owned(), hint: "ou cliquer pour choisir".to_owned() }
            }
            for p in shown {
                {
                    let label = p.label.as_ref().map_or_else(|| p.id.clone(), |l| tr(l, &lang, &p.id).to_owned());
                    let id = p.id.clone();
                    let current = values.read().get(&p.id).cloned().unwrap_or_default();
                    if p.ty == PortType::Bool {
                        rsx! {
                            Switch {
                                key: "{p.id}",
                                label,
                                checked: current == "true",
                                onchange: move |v: bool| {
                                    values.write().insert(id.clone(), v.to_string());
                                },
                            }
                        }
                    } else {
                        rsx! {
                            TextField {
                                key: "{p.id}",
                                label,
                                value: current,
                                oninput: move |v: String| {
                                    values.write().insert(id.clone(), v);
                                },
                            }
                        }
                    }
                }
            }
            Button { onclick: move |_| on_run.call(values.read().clone()), "Lancer" }
        }
    }
}
