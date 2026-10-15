use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use dioxus::prelude::*;
use kortex_contract::{Localized, PortType, ToolContract};
use kortex_i18n::Catalog;
use kortex_runtime::{FileData, ToolError, Value, ValueMap};

use crate::components::{Button, Card, DropZone, ModeSwitch, Progress, Select, Switch, TextField};
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
        ("fr", "download") => "Télécharger",
        ("fr", "err_missing_input") => "Ajoutez d'abord le contenu à traiter.",
        ("fr", "err_invalid_param") => "Valeur invalide.",
        ("fr", "err_cancelled") => "Annulé.",
        (_, "run") => "Run",
        (_, "running") => "Running…",
        (_, "copy") => "Copy",
        (_, "drop") => "Drop a file",
        (_, "drop_hint") => "or click to choose",
        (_, "result") => "Result",
        (_, "download") => "Download",
        (_, "err_missing_input") => "Add the content to process first.",
        (_, "err_invalid_param") => "Invalid value.",
        (_, "err_cancelled") => "Cancelled.",
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

/// Minimal base64 encoder (standard alphabet) for browser downloads.
fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(T[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn download_js(f: &FileData) -> String {
    format!(
        "(()=>{{const b=atob({data:?});const u=new Uint8Array(b.length);for(let i=0;i<b.length;i++)u[i]=b.charCodeAt(i);const a=document.createElement('a');a.href=URL.createObjectURL(new Blob([u],{{type:{mime:?}}}));a.download={name:?};a.click();}})()",
        data = base64(&f.bytes),
        mime = if f.mime.is_empty() {
            "application/octet-stream"
        } else {
            &f.mime
        },
        name = f.name,
    )
}

fn error_text(locale: &Catalog, lang: &str, code: &str) -> String {
    let key = format!("errors.{code}");
    let t = locale.t(&key);
    if t != key {
        return t.to_owned();
    }
    let fallback = s(lang, &format!("err_{code}"));
    if fallback.is_empty() {
        code.to_owned()
    } else {
        fallback.to_owned()
    }
}

/// Text output ready to display: `(label, shown text, raw text to copy)`.
/// A tool may translate an output with `[outputs] <id> = "Label"` and a value with
/// `[values] <id>_<value> = "Text"` (an empty string hides the output).
fn present(locale: &Catalog, id: &str, v: &Value) -> Option<(Option<String>, String, String)> {
    let raw = text_output(v)?;
    let value_key = format!("values.{id}_{raw}");
    let shown = locale.t(&value_key);
    let shown = if shown == value_key {
        raw.clone()
    } else {
        shown.to_owned()
    };
    if shown.is_empty() {
        return None;
    }
    let label_key = format!("outputs.{id}");
    let label = locale.t(&label_key);
    Some(((label != label_key).then(|| label.to_owned()), shown, raw))
}

fn output_files(v: &Value) -> Vec<FileData> {
    match v {
        Value::File(f) => vec![f.clone()],
        Value::Files(fs) => fs.clone(),
        _ => Vec::new(),
    }
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
pub fn AutoTool(
    contract: ToolContract,
    lang: String,
    locale: Catalog,
    runner: Runner,
    /// Variant id whose preset overrides param defaults (SEO variants).
    variant: Option<String>,
) -> Element {
    let mut mode = use_signal(Mode::default);
    let mut values = use_signal(|| {
        let mut v = contract
            .params
            .iter()
            .map(|p| (p.id.clone(), default_text(p.default.as_ref())))
            .collect::<ParamValues>();
        let preset = variant
            .as_deref()
            .and_then(|id| contract.variants.iter().find(|x| x.id == id))
            .and_then(|x| x.preset.as_ref());
        for (k, val) in preset.into_iter().flatten() {
            if v.contains_key(k) {
                v.insert(k.clone(), default_text(Some(val)));
            }
        }
        v
    });
    let mut text_inputs = use_signal(ParamValues::new);
    let mut files = use_signal(BTreeMap::<String, Vec<FileData>>::new);
    let mut running = use_signal(|| false);
    let mut result = use_signal(|| None::<Result<ValueMap, ToolError>>);

    let file_inputs: Vec<_> = contract
        .inputs
        .iter()
        .filter(|i| matches!(i.ty, PortType::File(_) | PortType::Files(_)))
        .cloned()
        .collect();
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

    let (run_label, running_label, copy_label, result_label, download_label) = (
        s(&lang, "run"),
        s(&lang, "running"),
        s(&lang, "copy"),
        s(&lang, "result"),
        s(&lang, "download"),
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
        Card { class: "k-stack".to_owned(),
            header { class: "k-tool-head",
                h2 { "{name}" }
                ModeSwitch {
                    mode: mode(),
                    onchange: move |m| mode.set(m),
                    labels: ["Simple".to_owned(), "Medium".to_owned(), "Pro".to_owned()],
                }
            }
            for i in file_inputs {
                {
                    let id = i.id.clone();
                    let multiple = matches!(i.ty, PortType::Files(_));
                    let names: Vec<String> = files
                        .read()
                        .get(&i.id)
                        .map(|v| v.iter().map(|f| format!("{} ({} Ko)", f.name, f.bytes.len() / 1024)).collect())
                        .unwrap_or_default();
                    rsx! {
                        div { key: "file-{i.id}",
                            DropZone {
                                label: s(&lang, "drop").to_owned(),
                                hint: s(&lang, "drop_hint").to_owned(),
                                multiple,
                                onfiles: move |picked: Vec<dioxus::html::FileData>| {
                                    let id = id.clone();
                                    spawn(async move {
                                        let mut read = Vec::new();
                                        for f in picked {
                                            if let Ok(bytes) = f.read_bytes().await {
                                                read.push(FileData {
                                                    name: f.name(),
                                                    mime: f.content_type().unwrap_or_default(),
                                                    bytes: bytes.to_vec(),
                                                });
                                            }
                                        }
                                        if !multiple {
                                            read.truncate(1);
                                        }
                                        files.write().insert(id, read);
                                    });
                                },
                            }
                            ul { class: "k-files",
                                for n in names {
                                    li { key: "{n}", "{n}" }
                                }
                            }
                        }
                    }
                }
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
                    if p.ty == PortType::Enum {
                        rsx! {
                            Select {
                                key: "{p.id}",
                                label,
                                value: current,
                                options: p.options.clone().unwrap_or_default(),
                                onchange: move |v: String| {
                                    values.write().insert(id.clone(), v);
                                },
                            }
                        }
                    } else if p.ty == PortType::Bool {
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
                            for file in output_files(&v) {
                                if file.mime.starts_with("image/") {
                                    img {
                                        key: "{id}-{file.name}-preview",
                                        class: "k-preview",
                                        alt: "{file.name}",
                                        src: "data:{file.mime};base64,{base64(&file.bytes)}",
                                    }
                                }
                                div { key: "{id}-{file.name}", class: "k-output",
                                    code { "{file.name}" }
                                    Button {
                                        onclick: {
                                            let js = download_js(&file);
                                            move |_| {
                                                document::eval(&js);
                                            }
                                        },
                                        "{download_label}"
                                    }
                                }
                            }
                            if let Some((label, text, raw)) = present(&locale, &id, &v) {
                                div { key: "{id}", class: "k-output-group",
                                    if let Some(label) = label {
                                        span { class: "k-muted", "{label}" }
                                    }
                                    div { class: "k-output",
                                        code { "{text}" }
                                        Button {
                                            ghost: true,
                                            onclick: move |_| {
                                                let js = format!("navigator.clipboard.writeText({raw:?})");
                                                document::eval(&js);
                                            },
                                            "{copy_label}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => {
                    let message = error_text(&locale, &lang, e.code);
                    rsx! {
                        p { class: "k-error", role: "alert", "{message}" }
                    }
                }
                None => rsx! {},
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_matches_reference() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
