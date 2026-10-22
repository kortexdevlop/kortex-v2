//! Tool contract (`tool.toml`): schema, port types, validation.
//! Site, Desk, Flow, Mind, Seek, Store and CLI read contracts only.

mod json_schema;
mod port;
mod schema;
/// Additive multi-operation contract; the deployed v1 ABI remains unchanged.
pub mod v2;
mod validate;

pub use json_schema::{port_schema, tool_definition};
pub use port::{PortType, PortTypeError};
pub use schema::{
    AppContract, Input, Limits, Localized, Output, Param, ParamLevel, Permissions, Search, Status,
    ToolContract, ToolMeta, UiKind, Variant,
};
pub use validate::{Issue, validate};

/// Languages every contract must provide.
pub const LANGS: [&str; 2] = ["fr", "en"];

/// Parse a contract from TOML text.
///
/// # Errors
/// Returns the TOML/serde error message if the text is not a valid contract.
pub fn parse(text: &str) -> Result<ToolContract, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

/// Parse an application with the same contract types as a tool.
///
/// # Errors
/// Returns a parse error for missing fields or an unknown top-level section.
pub fn parse_app(text: &str) -> Result<AppContract, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

/// Validate shared rules plus the mandatory installable application targets.
#[must_use]
pub fn validate_app(app: &AppContract) -> Vec<Issue> {
    let mut issues =
        validate::validate_contract(&app.as_tool_contract(), &["apps".to_owned()], false);
    if app.app.universe != "apps" || !app.app.id.starts_with("apps.") {
        issues.push(Issue(
            "app.id doit valoir apps.<nom>, universe = apps".into(),
        ));
    }
    if !["web", "desktop"]
        .iter()
        .all(|target| app.app.platforms.iter().any(|p| p == target))
    {
        issues.push(Issue(
            "une application installable doit déclarer web et desktop".into(),
        ));
    }
    issues
}
