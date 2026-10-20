//! The connectors Flow ships, read from `apps/flow/connectors/*/connector.toml`. Each action is a
//! tool of the Flow catalogue (`connector.<id>.<action>`); the contract format and the request
//! building live in `kortex_workflow::connector`.
use std::sync::OnceLock;

use kortex_workflow::connector::{self, Action, Connector, split_tool};

const SOURCES: [&str; 4] = [
    include_str!("../connectors/webhook/connector.toml"),
    include_str!("../connectors/telegram/connector.toml"),
    include_str!("../connectors/discord/connector.toml"),
    include_str!("../connectors/github/connector.toml"),
];

/// Every shipped connector. A contract that fails validation is a build mistake caught by the
/// tests, never silently dropped at run time (it is skipped here so the app still starts).
pub fn all() -> &'static [Connector] {
    static ALL: OnceLock<Vec<Connector>> = OnceLock::new();
    ALL.get_or_init(|| {
        SOURCES
            .iter()
            .filter_map(|text| connector::parse(text).ok())
            .collect()
    })
}

/// The connector and action behind a catalogue tool id, if it is one.
pub fn find(tool: &str) -> Option<(&'static Connector, &'static Action)> {
    let (id, action) = split_tool(tool)?;
    let connector = all().iter().find(|c| c.id == id)?;
    Some((connector, connector.action(action)?))
}

fn text<'a>(map: &'a std::collections::BTreeMap<String, String>, lang: &str) -> &'a str {
    map.get(lang)
        .or_else(|| map.get("en"))
        .map_or("", String::as_str)
}

/// Display name of an action: "Connector — action".
pub fn display_name(tool: &str, lang: &str) -> Option<String> {
    let (c, a) = find(tool)?;
    Some(format!("{} — {}", text(&c.name, lang), text(&a.name, lang)))
}

/// Label of an action parameter.
pub fn param_label(tool: &str, param: &str, lang: &str) -> Option<String> {
    let (_, a) = find(tool)?;
    let p = a.params.iter().find(|p| p.id == param)?;
    Some(text(&p.label, lang).to_owned())
}

/// Help text of the connector behind a tool.
pub fn help(tool: &str, lang: &str) -> Option<String> {
    let (c, _) = find(tool)?;
    Some(text(&c.help, lang).to_owned()).filter(|h| !h.is_empty())
}
