//! The library panel: "My workflows" (saved on this device) and ready-made templates.
use dioxus::prelude::*;

use crate::library::new_id;
use crate::templates;
use crate::ui::{Ctx, load_workflow};

fn need_label(l: &kortex_i18n::Catalog, need: &str) -> String {
    match need.strip_prefix("secret:") {
        Some(name) => format!("{} {name}", l.t("lib.needs_secret")),
        None => l.t(&format!("lib.needs_{need}")).to_owned(),
    }
}

pub(crate) fn library_panel(ctx: Ctx, l: &'static kortex_i18n::Catalog, lang: &str) -> Element {
    let Ctx {
        mut library,
        mut current_id,
        mut lib_tab,
        mut lib_search,
        mut confirm_delete,
        workflow,
        ..
    } = ctx;
    let tab = lib_tab();
    let query = lib_search().trim().to_lowercase();
    let current = workflow();
    let here = current_id();
    let saved: Vec<_> = library
        .read()
        .visible()
        .into_iter()
        .filter(|s| query.is_empty() || s.name.to_lowercase().contains(&query))
        .cloned()
        .collect();
    let lang_owned = lang.to_owned();
    let shown: Vec<_> = templates::all()
        .iter()
        .filter(|t| {
            query.is_empty()
                || t.name(lang).to_lowercase().contains(&query)
                || t.description(lang).to_lowercase().contains(&query)
        })
        .collect();
    let suffix = l.t("lib.copy_suffix").to_owned();
    rsx! {
        section { class: "flow-library", "data-panel": "library", "aria-label": l.t("lib.title"),
            div { class: "flow-lib-head",
                div { class: "flow-tabs", role: "tablist",
                    for (id , label) in [("mine", "lib.mine"), ("templates", "lib.templates")] {
                        button {
                            key: "{id}",
                            class: "k-btn",
                            "data-variant": if tab == id { "" } else { "ghost" },
                            r#type: "button",
                            role: "tab",
                            "aria-selected": "{tab == id}",
                            "data-tab": "{id}",
                            onclick: move |_| {
                                lib_tab.set(id.to_owned());
                                lib_search.set(String::new());
                            },
                            {l.t(label)}
                        }
                    }
                }
                input {
                    class: "k-input",
                    r#type: "search",
                    placeholder: l.t("lib.search"),
                    "aria-label": l.t("lib.search"),
                    value: "{lib_search}",
                    oninput: move |e| lib_search.set(e.value()),
                }
            }
            if tab == "mine" {
                div { class: "flow-lib-actions",
                    button {
                        class: "k-btn",
                        r#type: "button",
                        "data-action": "save-workflow",
                        disabled: current.nodes.is_empty(),
                        onclick: {
                            let current = current.clone();
                            move |_| {
                                let name = if current.name.trim().is_empty() { l.t("lib.untitled").to_owned() } else { current.name.clone() };
                                let id = library.write().save(current_id.peek().as_deref(), &name, &current, kortex_platform::now_ms(), new_id);
                                current_id.set(Some(id));
                            }
                        },
                        {if here.is_some() { l.t("lib.update") } else { l.t("lib.save") }}
                    }
                    button {
                        class: "k-btn",
                        "data-variant": "ghost",
                        r#type: "button",
                        "data-action": "new-workflow",
                        onclick: move |_| load_workflow(ctx, crate::engine::Workflow::default(), None, l.t("lib.untitled")),
                        {l.t("lib.new")}
                    }
                }
                if saved.is_empty() {
                    p { class: "flow-hint", {l.t("lib.empty")} }
                }
                ul { class: "flow-lib-list",
                    for item in saved {
                        li { key: "{item.id}", class: "flow-lib-item", "data-saved": "{item.id}", "data-current": "{here.as_deref() == Some(item.id.as_str())}",
                            div { class: "flow-lib-main",
                                strong { "{item.name}" }
                                span { class: "flow-hint", "{item.workflow.nodes.len()} {l.t(\"lib.nodes\")}" }
                            }
                            div { class: "flow-lib-buttons",
                                button {
                                    class: "k-btn",
                                    r#type: "button",
                                    "data-action": "open",
                                    onclick: {
                                        let (item, ) = (item.clone(), );
                                        move |_| load_workflow(ctx, item.workflow.clone(), Some(item.id.clone()), l.t("lib.untitled"))
                                    },
                                    {l.t("lib.open")}
                                }
                                button {
                                    class: "k-btn",
                                    "data-variant": "ghost",
                                    r#type: "button",
                                    "data-action": "duplicate",
                                    onclick: {
                                        let (id, suffix) = (item.id.clone(), suffix.clone());
                                        move |_| {
                                            library.write().duplicate(&id, &suffix, kortex_platform::now_ms(), new_id);
                                        }
                                    },
                                    {l.t("lib.duplicate")}
                                }
                                if confirm_delete().as_deref() == Some(item.id.as_str()) {
                                    button {
                                        class: "k-btn",
                                        r#type: "button",
                                        "data-action": "confirm-delete",
                                        onclick: {
                                            let id = item.id.clone();
                                            move |_| {
                                                library.write().remove(&id, kortex_platform::now_ms());
                                                if current_id.peek().as_deref() == Some(id.as_str()) {
                                                    current_id.set(None);
                                                }
                                                confirm_delete.set(None);
                                            }
                                        },
                                        {l.t("lib.confirm")}
                                    }
                                } else {
                                    button {
                                        class: "k-btn",
                                        "data-variant": "ghost",
                                        r#type: "button",
                                        "data-action": "delete",
                                        onclick: {
                                            let id = item.id.clone();
                                            move |_| confirm_delete.set(Some(id.clone()))
                                        },
                                        {l.t("lib.delete")}
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                p { class: "flow-hint", {l.t("lib.templates_help")} }
                div { class: "flow-lib-grid",
                    for template in shown {
                        article { key: "{template.id}", class: "flow-template", "data-template": "{template.id}",
                            strong { "{template.name(lang)}" }
                            p { "{template.description(lang)}" }
                            if !template.needs.is_empty() {
                                p { class: "flow-needs",
                                    for need in template.needs.clone() {
                                        span { key: "{need}", class: "app-badge", "{need_label(l, &need)}" }
                                    }
                                }
                            }
                            button {
                                class: "k-btn",
                                r#type: "button",
                                "data-action": "use-template",
                                onclick: {
                                    let (id, lang) = (template.id.clone(), lang_owned.clone());
                                    move |_| {
                                        if let Some(t) = templates::all().iter().find(|t| t.id == id) {
                                            load_workflow(ctx, t.instantiate(&lang), None, l.t("lib.untitled"));
                                            let mut open = ctx.lib_open;
                                            open.set(false);
                                        }
                                    }
                                },
                                {l.t("lib.use")}
                            }
                        }
                    }
                }
            }
        }
    }
}
