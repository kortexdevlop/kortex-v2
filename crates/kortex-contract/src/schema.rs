use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::PortType;

pub type Localized = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Draft,
    Beta,
    Stable,
    Deprecated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UiKind {
    Auto,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamLevel {
    Simple,
    Medium,
    Pro,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolMeta {
    pub id: String,
    pub version: String,
    pub universe: String,
    pub status: Status,
    pub ui: UiKind,
    /// Run the web engine in an isolated Worker; desktop keeps its native runner.
    #[serde(default)]
    pub web_worker: bool,
    /// Web builds of this application ship the headless engine of every tool under `engines/`
    /// (Mind and Flow run tools in Web Workers of their own origin).
    #[serde(default, skip_serializing_if = "is_false")]
    pub bundles_engines: bool,
    pub platforms: Vec<String>,
    #[serde(default)]
    pub offline: bool,
    #[serde(default)]
    pub ai: bool,
    #[serde(default = "default_priority")]
    pub viral_priority: u8,
    pub name: Localized,
    pub tagline: Localized,
    pub slug: Localized,
    /// Former slugs per language, kept as redirects.
    #[serde(default)]
    pub aliases: BTreeMap<String, Vec<String>>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(value: &bool) -> bool {
    !*value
}

const fn default_priority() -> u8 {
    3
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Search {
    #[serde(default)]
    pub keywords: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub intents: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub related: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Limits {
    pub max_input_bytes: Option<String>,
    pub max_batch: Option<u32>,
    pub timeout_s: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Input {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: PortType,
    #[serde(default)]
    pub required: bool,
    pub max_len: Option<u32>,
    /// Show a multi-line text area (documents, JSON, lists) instead of a single-line field.
    #[serde(default)]
    pub multiline: bool,
    pub label: Option<Localized>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Param {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: PortType,
    pub level: Option<ParamLevel>,
    pub default: Option<toml::Value>,
    pub range: Option<Vec<f64>>,
    pub options: Option<Vec<String>>,
    pub label: Option<Localized>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Output {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: PortType,
    pub formats: Option<Vec<String>>,
    pub default_format: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Permissions {
    pub network: bool,
    pub filesystem: String,
    pub clipboard: String,
    #[serde(default)]
    pub sensitive: bool,
    pub privacy: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Variant {
    pub id: String,
    pub slug: Localized,
    pub name: Localized,
    pub preset: Option<toml::Table>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ToolContract {
    pub schema: u32,
    pub tool: ToolMeta,
    #[serde(default)]
    pub search: Search,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub inputs: Vec<Input>,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default)]
    pub outputs: Vec<Output>,
    pub permissions: Permissions,
    #[serde(default)]
    pub variants: Vec<Variant>,
}

/// Installable applications use the same metadata, ports and permission types as tools.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppContract {
    pub schema: u32,
    pub app: ToolMeta,
    #[serde(default)]
    pub search: Search,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub inputs: Vec<Input>,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default)]
    pub outputs: Vec<Output>,
    pub permissions: Permissions,
    #[serde(default)]
    pub variants: Vec<Variant>,
}

impl AppContract {
    /// A common validation view; no second implementation of port/permission rules.
    #[must_use]
    pub fn as_tool_contract(&self) -> ToolContract {
        ToolContract {
            schema: self.schema,
            tool: self.app.clone(),
            search: self.search.clone(),
            limits: self.limits.clone(),
            inputs: self.inputs.clone(),
            params: self.params.clone(),
            outputs: self.outputs.clone(),
            permissions: self.permissions.clone(),
            variants: self.variants.clone(),
        }
    }
}
