use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use dioxus::prelude::*;
use kortex_contract::{Localized, PortType, ToolContract};
use kortex_i18n::Catalog;
use kortex_runtime::{ToolError, Value, ValueMap};

use crate::components::{Button, Card, DropZone, ModeSwitch, Progress, Switch, TextField};
use crate::{Mode, visible_params};

/// Current param values keyed by param id (stringly typed at the UI boundary).
pub type ParamValues = BTreeMap<String, String>;

pub type RunFuture = Pin<Box<dyn Future<Output = Result<ValueMap, ToolError>>>>;
/// Host-provided entry: `(inputs, params) -> outputs`, usually `run_map(&i, &p, &mut Cx::new())`.
/// All runners compare equal: a host never swaps its runner while the UI is mounted.
#[derive(Clone, Copy)]
pub struct Runner(pub fn(ValueMap, ValueMap) -> RunFuture);

impl PartialEq for Runner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

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

/// Generic UI strings (tool-specific ones live in the tool's `locales/`).
fn s(lang: &str, key: &str) -> &'static str {
    match (lang, key) {
        ("fr", "run") => "Lancer",
        ("fr", "running") => "En cours…",
        ("fr", "copy") => "Copier",
        ("fr", "drop") => "Déposer un fichier",
        ("fr", "drop_hint") => "ou cliquer pour choisir",
        ("fr", "result") => "Résultat",
        (_, "run") => "Run",
        (_, "running") => "Running…",
        (_, "copy") => "Copy",
        (_, "drop") => "Drop a file",
        (_, "drop_hint") => "or click to choose",
        (_, "result") => "Result",
        _ => "",
    }
}

/// Convert UI strings into typed values following the contract's param types.
/// Unparseable numbers are skipped so the contract default applies.
#[must_use]
pub fn to_values(c: &ToolContract, strings: &ParamValues) -> ValueMap {
    let mut out = ValueMap::new();
    for p in &c.params {
        let Some(raw) = strings.get(&p.id) else {
            continue;
        };
        let v = match p.ty {
            PortType::Integer => raw.trim().parse().ok().map(Value::Int),
            PortType::Number | PortType::Duration | PortType::Range => {
                raw.trim().parse().ok().map(Value::Number)
            }
            PortType::Bool => Some(Value::Bool(raw == "true")),
            _ => Some(Value::Text(raw.clone())),
        };
        if let Some(v) = v {
            out.insert(p.id.clone(), v);
        }
    }
    out
}

fn text_output(v: &Value) -> Option<String> {
    match v {
        Value::Text(t) => Some(t.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Int(i) => Some(i.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// UI generated entirely from a tool contract: inputs, params by mode, run button, result.
#[component]
pub fn AutoTool(contract: ToolContract, lang: String, locale: Catalog, runner: Runner) -> Element {
    let mut mode = use_signal(Mode::default);
    let mut values = use_signal(|| {
        contract
            .params
            .iter()
            .map(|p| (p.id.clone(), default_text(p.default.as_ref())))
            .collect::<ParamValues>()
    });
    let mut text_inputs = use_signal(ParamValues::new);
    let mut running = use_signal(|| false);
    let mut result = use_signal(|| None::<Result<ValueMap, ToolError>>);

    let has_file_input = contract
        .inputs
        .iter()
        .any(|i| matches!(i.ty, PortType::File(_) | PortType::Files(_)));
    let name = tr(&contract.tool.name, &lang, &contract.tool.id).to_owned();
    let shown: Vec<_> = visible_params(&contract, mode())
        .into_iter()
        .cloned()
        .collect();
    let text_fields: Vec<_> = contract
        .inputs
        .iter()
        .filter(|i| i.ty == PortType::Text)
        .cloned()
        .collect();

    let (run_label, running_label, copy_label, result_label) = (
        s(&lang, "run"),
        s(&lang, "running"),
        s(&lang, "copy"),
        s(&lang, "result"),
    );
    let run_contract = contract.clone();
    let run = move |_| {
        if running() {
            return;
        }
        let params = to_values(&run_contract, &values.read());
        let inputs: ValueMap = run_contract
            .inputs
            .iter()
            .filter(|i| i.ty == PortType::Text)
            .map(|i| {
                (
                    i.id.clone(),
                    Value::Text(text_inputs.read().get(&i.id).cloned().unwrap_or_default()),
                )
            })
            .collect();
        running.set(true);
        spawn(async move {
            let r = (runner.0)(inputs, params).await;
            result.set(Some(r));
            running.set(false);
        });
    };

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
                DropZone { label: s(&lang, "drop").to_owned(), hint: s(&lang, "drop_hint").to_owned() }
            }
            for i in text_fields {
                {
                    let label = i.label.as_ref().map_or_else(|| i.id.clone(), |l| tr(l, &lang, &i.id).to_owned());
                    let id = i.id.clone();
                    let current = text_inputs.read().get(&i.id).cloned().unwrap_or_default();
                    rsx! {
                        TextField {
                            key: "in-{i.id}",
                            label,
                            value: current,
                            oninput: move |v: String| {
                                text_inputs.write().insert(id.clone(), v);
                            },
                        }
                    }
                }
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
            div { class: "k-actions",
                Button { disabled: running(), onclick: run,
                    if running() {
                        "{running_label}"
                    } else {
                        "{run_label}"
                    }
                }
            }
            if running() {
                Progress { value: 0.5 }
            }
            match result() {
                Some(Ok(map)) => rsx! {
                    section { class: "k-result", "aria-live": "polite",
                        h3 { "{result_label}" }
                        for (id , v) in map {
                            if let Some(text) = text_output(&v) {
                                div { key: "{id}", class: "k-output",
                                    code { "{text}" }
                                    Button {
                                        ghost: true,
                                        onclick: {
                                            let text = text.clone();
                                            move |_| {
                                                let js = format!("navigator.clipboard.writeText({text:?})");
                                                document::eval(&js);
                                            }
                                        },
                                        "{copy_label}"
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => {
                    let message = locale.t(&format!("errors.{}", e.code)).to_owned();
                    rsx! {
                        p { class: "k-error", role: "alert", "{message}" }
                    }
                }
                None => rsx! {},
            }
        }
    }
}
