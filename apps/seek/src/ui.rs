//! Kortex Seek UI: one search box over the contract catalogue, shared by web and desktop.
use std::sync::OnceLock;

use dioxus::prelude::*;
use kortex_shell::{self as shell, Frame};

use crate::engine::{Doc, Hit, search};

const RESULT_LIMIT: usize = 8;

fn locale(lang: &str) -> &'static kortex_i18n::Catalog {
    static FR: OnceLock<kortex_i18n::Catalog> = OnceLock::new();
    static EN: OnceLock<kortex_i18n::Catalog> = OnceLock::new();
    if lang == "fr" {
        FR.get_or_init(|| {
            kortex_i18n::Catalog::from_toml(include_str!("../locales/fr.toml")).unwrap_or_default()
        })
    } else {
        EN.get_or_init(|| {
            kortex_i18n::Catalog::from_toml(include_str!("../locales/en.toml")).unwrap_or_default()
        })
    }
}

/// Search documents for every tool and each of its SEO variants, built once from the registry.
fn docs() -> &'static [Doc] {
    static DOCS: OnceLock<Vec<Doc>> = OnceLock::new();
    DOCS.get_or_init(|| {
        let mut docs = Vec::new();
        for entry in shell::catalogue() {
            let base = Doc {
                id: entry.tool.id.clone(),
                variant: None,
                universe: entry.tool.universe.clone(),
                names: entry.tool.name.clone(),
                tool_names: Default::default(),
                keywords: entry.search.keywords.clone(),
                intents: entry.search.intents.clone(),
            };
            for variant in &entry.variants {
                docs.push(Doc {
                    variant: Some(variant.id.clone()),
                    names: variant.name.clone(),
                    tool_names: entry.tool.name.clone(),
                    ..base.clone()
                });
            }
            docs.push(base);
        }
        docs
    })
}

/// Example requests taken from the most popular tools' own declared intents.
fn suggestions(lang: &str) -> Vec<String> {
    let mut entries: Vec<_> = shell::catalogue().iter().collect();
    entries.sort_by_key(|e| (e.tool.viral_priority, e.tool.id.clone()));
    entries
        .into_iter()
        .filter_map(|e| e.search.intents.get(lang).and_then(|i| i.first().cloned()))
        .take(4)
        .collect()
}

struct Row {
    id: String,
    title: String,
    tagline: String,
    universe: String,
    href: String,
}

fn row(hit: &Hit, lang: &str) -> Option<Row> {
    let entry = shell::find(&hit.id)?;
    let variant = hit
        .variant
        .as_deref()
        .and_then(|v| entry.variants.iter().find(|x| x.id == v));
    let pick = |map: &std::collections::BTreeMap<String, String>| {
        map.get(lang).or_else(|| map.get("en")).cloned()
    };
    let slug = pick(variant.map_or(&entry.tool.slug, |v| &v.slug))?;
    Some(Row {
        id: hit.id.clone(),
        title: pick(variant.map_or(&entry.tool.name, |v| &v.name))?,
        tagline: pick(&entry.tool.tagline).unwrap_or_default(),
        universe: entry.tool.universe.clone(),
        href: format!("{}{slug}?open=1", shell::url("root", lang)),
    })
}

#[component]
pub fn App() -> Element {
    let appearance = shell::use_appearance();
    let lang = (appearance.lang)();
    let l = locale(&lang);
    let mut query = use_signal(String::new);
    let mut launch_error = use_signal(|| false);
    let hits = search(docs(), &query(), &lang, RESULT_LIMIT);
    let rows: Vec<Row> = hits.iter().filter_map(|h| row(h, &lang)).collect();
    let typed = !query().trim().is_empty();
    let count = rows.len().to_string();
    rsx! {
        Frame { title: l.t("ui.title"), subtitle: l.t("ui.subtitle"),
            style { {include_str!("seek.css")} }
            label { class: "k-field",
                span { class: "k-muted", {l.t("ui.label")} }
                input {
                    class: "k-input app-search",
                    r#type: "search",
                    autofocus: true,
                    placeholder: l.t("ui.placeholder"),
                    value: "{query}",
                    oninput: move |e| {
                        query.set(e.value());
                        launch_error.set(false);
                    },
                }
            }
            if launch_error() {
                p { class: "app-note", role: "alert", {l.t("ui.not_installed")} }
            }
            if !typed {
                div { class: "seek-hints",
                    span { class: "k-muted", {l.t("ui.try")} }
                    for hint in suggestions(&lang) {
                        button {
                            class: "k-btn",
                            "data-variant": "ghost",
                            r#type: "button",
                            onclick: {
                                let hint = hint.clone();
                                move |_| query.set(hint.clone())
                            },
                            "{hint}"
                        }
                    }
                }
            } else if rows.is_empty() {
                p { class: "app-note", role: "status", {l.t("ui.empty")} }
            } else {
                p { class: "k-muted", role: "status", "{count} {l.t(\"ui.results\")}" }
                ul { class: "app-list",
                    for row in rows {
                        li { key: "{row.href}", class: "app-tool",
                            div { class: "app-tool-info",
                                strong { "{row.title}" }
                                small { "{row.tagline}" }
                            }
                            span { class: "app-badge", "{row.universe}" }
                            a {
                                class: "k-btn",
                                href: "{row.href}",
                                onclick: {
                                    let id = row.id.clone();
                                    move |event| {
                                        if !cfg!(target_arch = "wasm32") {
                                            event.prevent_default();
                                            launch_error.set(!shell::launch_installed(&id));
                                        }
                                    }
                                },
                                {l.t("ui.open")}
                            }
                        }
                    }
                }
            }
        }
    }
}
