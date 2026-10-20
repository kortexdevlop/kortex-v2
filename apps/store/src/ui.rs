//! Kortex Store: logic and presentational components (the window itself is in `main.rs`).
use crate::engine::ToolStatus;
use dioxus::prelude::*;

use crate::engine::{Action, display_name, primary_action};
use kortex_ui::Button;

pub struct Texts {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub search: &'static str,
    pub all: &'static str,
    pub install: &'static str,
    pub update: &'static str,
    pub update_all: &'static str,
    pub open: &'static str,
    pub remove: &'static str,
    pub installed: &'static str,
    pub unavailable: &'static str,
    pub busy: &'static str,
    pub empty: &'static str,
    pub loading: &'static str,
    pub retry: &'static str,
    pub version: &'static str,
    pub new_version: &'static str,
}
fn catalog(lang: &str) -> &'static kortex_i18n::Catalog {
    static FR: std::sync::OnceLock<kortex_i18n::Catalog> = std::sync::OnceLock::new();
    static EN: std::sync::OnceLock<kortex_i18n::Catalog> = std::sync::OnceLock::new();
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

/// One `[ui]` text of the Store locales.
#[must_use]
pub fn text(lang: &str, key: &str) -> String {
    catalog(lang).t(&format!("ui.{key}")).to_owned()
}

#[must_use]
pub fn texts(lang: &str) -> Texts {
    let catalog = catalog(lang);
    Texts {
        title: catalog.t("ui.title"),
        subtitle: catalog.t("ui.subtitle"),
        search: catalog.t("ui.search"),
        all: catalog.t("ui.all"),
        install: catalog.t("ui.install"),
        update: catalog.t("ui.update"),
        update_all: catalog.t("ui.update_all"),
        open: catalog.t("ui.open"),
        remove: catalog.t("ui.remove"),
        installed: catalog.t("ui.installed"),
        unavailable: catalog.t("ui.unavailable"),
        busy: catalog.t("ui.busy"),
        empty: catalog.t("ui.empty"),
        loading: catalog.t("ui.loading"),
        retry: catalog.t("ui.retry"),
        version: catalog.t("ui.version"),
        new_version: catalog.t("ui.new_version"),
    }
}

#[component]
pub fn ToolRow(
    status: ToolStatus,
    lang: String,
    busy: bool,
    onaction: EventHandler<(String, Action)>,
) -> Element {
    let t = texts(&lang);
    let name = display_name(&status, &lang);
    let id = status.id.clone();
    let primary = primary_action(&status);
    let label = |a: Action| match a {
        Action::Install => t.install,
        Action::Update => t.update,
        Action::Open => t.open,
        Action::Remove => t.remove,
    };
    let state = match (&status.installed, status.update_available, status.supported) {
        (_, _, false) if status.installed.is_none() => t.unavailable.to_owned(),
        (Some(v), true, _) => format!("{} {v} → {} {}", t.version, t.new_version, status.available),
        (Some(v), false, _) => format!("{} · {} {v}", t.installed, t.version),
        (None, _, _) => format!("{} {}", t.version, status.available),
    };
    rsx! {
        li { class: "k-card store-row", "data-id": "{status.id}", "data-universe": "{status.universe}",
            div { class: "store-info",
                strong { "{name}" }
                span { class: "k-muted", "{state}" }
            }
            div { class: "store-actions",
                if let Some(a) = primary {
                    Button {
                        disabled: busy,
                        onclick: {
                            let id = id.clone();
                            move |_| onaction.call((id.clone(), a))
                        },
                        if busy { "{t.busy}" } else { "{label(a)}" }
                    }
                }
                if status.installed.is_some() {
                    Button {
                        ghost: true,
                        disabled: busy,
                        onclick: move |_| onaction.call((id.clone(), Action::Remove)),
                        "{t.remove}"
                    }
                }
            }
        }
    }
}

/// Catalogue list (presentational: state lives in the app).
#[component]
pub fn Catalogue(
    items: Vec<ToolStatus>,
    lang: String,
    busy: Vec<String>,
    onaction: EventHandler<(String, Action)>,
) -> Element {
    let t = texts(&lang);
    rsx! {
        if items.is_empty() {
            p { class: "k-muted", "{t.empty}" }
        } else {
            ul { class: "store-list",
                for s in items {
                    ToolRow { key: "{s.id}", busy: busy.contains(&s.id), status: s, lang: lang.clone(), onaction }
                }
            }
        }
    }
}

pub const STORE_CSS: &str = "
.store { max-width: 760px; margin: 0 auto; padding: var(--k-space-5) var(--k-space-4); display: grid; gap: var(--k-space-4); }
.store-bar { display: flex; gap: var(--k-space-3); flex-wrap: wrap; align-items: center; }
.store-bar .k-input { flex: 1; min-width: 200px; }
.store-chips { display: flex; gap: var(--k-space-2); flex-wrap: wrap; }
.store-chip { border: 1px solid var(--k-border); background: var(--k-surface); color: var(--k-text-2); border-radius: var(--k-radius-control); min-height: 36px; padding: 0 var(--k-space-4); font: inherit; cursor: pointer; }
.store-chip[aria-pressed=\"true\"] { background: var(--k-accent); color: var(--k-accent-text); border-color: transparent; }
.store-list { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--k-space-3); }
.store-row { display: flex; align-items: center; justify-content: space-between; gap: var(--k-space-4); }
.store-info { display: grid; gap: var(--k-space-1); }
.store-actions { display: flex; gap: var(--k-space-2); flex-shrink: 0; }
.store-toast { position: fixed; left: 50%; bottom: var(--k-space-5); transform: translateX(-50%); max-width: 90vw; padding: var(--k-space-3) var(--k-space-5); border-radius: var(--k-radius-card); background: var(--k-text); color: var(--k-bg); box-shadow: var(--k-shadow-4); }
";
