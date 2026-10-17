//! Pure Store catalogue rules, independent of Dioxus and platform access.
use std::collections::BTreeMap;

/// Local catalogue state. Browser availability never implies a native installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolStatus {
    pub id: String,
    pub name: BTreeMap<String, String>,
    pub universe: String,
    pub available: String,
    pub installed: Option<String>,
    pub update_available: bool,
    pub supported: bool,
}

/// What a row's primary button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Update,
    Open,
    Remove,
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
