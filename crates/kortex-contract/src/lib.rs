//! Tool contract (`tool.toml`): schema, port types, validation.
//! Site, Desk, Flow, Mind, Seek, Store and CLI read contracts only.

mod port;
mod schema;
mod validate;

pub use port::{PortType, PortTypeError};
pub use schema::{
    Input, Limits, Localized, Output, Param, ParamLevel, Permissions, Search, Status, ToolContract,
    ToolMeta, UiKind, Variant,
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
