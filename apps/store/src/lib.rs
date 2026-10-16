//! Kortex Store: logic and presentational components (the window itself is in `main.rs`).
use dioxus::prelude::*;
use kortex_pkg::ToolStatus;

/// What a row's primary button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Update,
    Open,
    Remove,
}

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

#[must_use]
pub fn texts(lang: &str) -> Texts {
    if lang == "fr" {
        Texts {
            title: "Kortex Store",
            subtitle: "Installez, mettez à jour et supprimez vos outils. Chaque paquet est vérifié (signature et empreinte) avant installation.",
            search: "Rechercher un outil",
            all: "Tous",
            install: "Installer",
            update: "Mettre à jour",
            update_all: "Tout mettre à jour",
            open: "Ouvrir",
            remove: "Supprimer",
            installed: "Installé",
            unavailable: "Indisponible sur cette plateforme",
            busy: "En cours…",
            empty: "Aucun outil ne correspond.",
            loading: "Chargement du catalogue…",
            retry: "Réessayer",
            version: "version",
            new_version: "nouvelle version",
        }
    } else {
        Texts {
            title: "Kortex Store",
            subtitle: "Install, update and remove your tools. Every package is verified (signature and checksum) before installation.",
            search: "Search tools",
            all: "All",
            install: "Install",
            update: "Update",
            update_all: "Update all",
            open: "Open",
            remove: "Remove",
            installed: "Installed",
            unavailable: "Not available on this platform",
            busy: "Working…",
            empty: "No tool matches.",
            loading: "Loading catalogue…",
            retry: "Retry",
            version: "version",
            new_version: "new version",
        }
    }
}

fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' => 'a',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'ù' | 'û' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

#[must_use]
pub fn display_name(s: &ToolStatus, lang: &str) -> String {
    s.name
        .get(lang)
        .or_else(|| s.name.get("en"))
        .cloned()
        .unwrap_or_else(|| s.id.clone())
}

/// Tools matching the search text (every word, accents ignored) and the universe filter.
#[must_use]
pub fn visible<'a>(
    all: &'a [ToolStatus],
    query: &str,
    universe: Option<&str>,
    lang: &str,
) -> Vec<&'a ToolStatus> {
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_owned).collect();
    all.iter()
        .filter(|s| universe.is_none_or(|u| s.universe == u))
        .filter(|s| {
            let hay = fold(&format!("{} {}", display_name(s, lang), s.id));
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}

#[must_use]
pub fn universes(all: &[ToolStatus]) -> Vec<String> {
    all.iter()
        .map(|s| s.universe.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The primary action for a row, if any (`None` = nothing to offer, e.g. no build for this OS).
#[must_use]
pub fn primary_action(s: &ToolStatus) -> Option<Action> {
    match (&s.installed, s.update_available, s.supported) {
        (Some(_), true, true) => Some(Action::Update),
        (Some(_), _, _) => Some(Action::Open),
        (None, _, true) => Some(Action::Install),
        (None, _, false) => None,
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
                    button {
                        class: "k-btn",
                        r#type: "button",
                        disabled: busy,
                        onclick: {
                            let id = id.clone();
                            move |_| onaction.call((id.clone(), a))
                        },
                        if busy { "{t.busy}" } else { "{label(a)}" }
                    }
                }
                if status.installed.is_some() {
                    button {
                        class: "k-btn",
                        "data-variant": "ghost",
                        r#type: "button",
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
.store-chip { border: 1px solid var(--k-border); background: var(--k-surface); color: var(--k-text-2); border-radius: 999px; min-height: 36px; padding: 0 var(--k-space-4); font: inherit; cursor: pointer; }
.store-chip[aria-pressed=\"true\"] { background: var(--k-accent); color: var(--k-accent-text); border-color: transparent; }
.store-list { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--k-space-3); }
.store-row { display: flex; align-items: center; justify-content: space-between; gap: var(--k-space-4); }
.store-info { display: grid; gap: var(--k-space-1); }
.store-actions { display: flex; gap: var(--k-space-2); flex-shrink: 0; }
.store-toast { position: fixed; left: 50%; bottom: var(--k-space-5); transform: translateX(-50%); max-width: 90vw; padding: var(--k-space-3) var(--k-space-5); border-radius: var(--k-radius-card); background: var(--k-text); color: var(--k-bg); box-shadow: var(--k-shadow-4); }
";
