//! JSON Schema tool definitions derived from contracts (for Mind, Seek and any future LLM host).
//!
//! The contract stays the single source of truth: nothing here is hand-written per tool. Files
//! are exposed as opaque references (`x-kortex-file-mime`), never as content: a file is only
//! sent to a model with the user's explicit per-file consent (RULES 6.4).
use serde_json::{Map, Value, json};

use crate::{Input, Localized, Param, PortType, ToolContract};

const DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";

fn text<'a>(localized: &'a Localized, lang: &str) -> Option<&'a str> {
    localized
        .get(lang)
        .or_else(|| localized.get("en"))
        .map(String::as_str)
}

fn toml_to_json(value: &toml::Value) -> Option<Value> {
    match value {
        toml::Value::String(s) => Some(json!(s)),
        toml::Value::Integer(i) => Some(json!(i)),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f).map(Value::Number),
        toml::Value::Boolean(b) => Some(json!(b)),
        _ => None,
    }
}

fn file_reference(mime: &str) -> Value {
    json!({
        "type": "string",
        "description": "Reference to a file the user selected on their device; content is never part of the call.",
        "x-kortex-file-mime": mime,
    })
}

/// Schema of one port value; constraints that only exist on inputs or params are added by callers.
#[must_use]
pub fn port_schema(ty: &PortType) -> Value {
    match ty {
        PortType::Text | PortType::RichText => json!({"type": "string"}),
        PortType::Number => json!({"type": "number"}),
        PortType::Integer => json!({"type": "integer"}),
        PortType::Bool => json!({"type": "boolean"}),
        PortType::Enum => json!({"type": "string"}),
        PortType::Color => json!({"type": "string", "pattern": "^#[0-9a-fA-F]{6}$"}),
        PortType::Date => json!({"type": "string", "format": "date"}),
        PortType::DateTime => json!({"type": "string", "format": "date-time"}),
        PortType::Duration => json!({"type": "string", "x-kortex-type": "duration"}),
        PortType::Range => {
            json!({"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2})
        }
        PortType::Json => json!({}),
        PortType::Table => json!({
            "type": "object",
            "properties": {
                "columns": {"type": "array", "items": {"type": "string"}},
                "rows": {"type": "array", "items": {"type": "array", "items": {"type": "string"}}},
            },
            "required": ["columns", "rows"],
            "additionalProperties": false,
        }),
        PortType::Secret => json!({"type": "string", "writeOnly": true, "x-kortex-secret": true}),
        PortType::Url => json!({"type": "string", "format": "uri"}),
        PortType::File(mime) => file_reference(mime),
        PortType::Files(mime) => json!({"type": "array", "items": file_reference(mime)}),
    }
}

fn set(schema: &mut Value, key: &str, value: Value) {
    if let Value::Object(map) = schema {
        map.insert(key.to_owned(), value);
    }
}

fn input_schema(input: &Input, lang: &str) -> Value {
    let mut schema = port_schema(&input.ty);
    if let (Some(max), PortType::Text | PortType::RichText) = (input.max_len, &input.ty) {
        set(&mut schema, "maxLength", json!(max));
    }
    if let Some(label) = input.label.as_ref().and_then(|l| text(l, lang)) {
        set(&mut schema, "description", json!(label));
    }
    schema
}

fn param_schema(param: &Param, lang: &str) -> Value {
    let mut schema = port_schema(&param.ty);
    if let (PortType::Number | PortType::Integer, Some([min, max])) =
        (&param.ty, param.range.as_deref())
    {
        set(&mut schema, "minimum", json!(min));
        set(&mut schema, "maximum", json!(max));
    }
    if let (PortType::Enum, Some(options)) = (&param.ty, &param.options) {
        set(&mut schema, "enum", json!(options));
    }
    if let Some(default) = param.default.as_ref().and_then(toml_to_json) {
        set(&mut schema, "default", default);
    }
    if let Some(label) = param.label.as_ref().and_then(|l| text(l, lang)) {
        set(&mut schema, "description", json!(label));
    }
    schema
}

/// Tool definition for `lang` (`fr`/`en`): `name`, `description`, `input_schema` plus the
/// declared permissions and outputs under `x-kortex` so a host can ask for confirmation.
#[must_use]
pub fn tool_definition(contract: &ToolContract, lang: &str) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for input in &contract.inputs {
        properties.insert(input.id.clone(), input_schema(input, lang));
        if input.required {
            required.push(json!(input.id));
        }
    }
    for param in &contract.params {
        properties.insert(param.id.clone(), param_schema(param, lang));
    }
    let outputs: Vec<Value> = contract
        .outputs
        .iter()
        .map(|o| json!({"id": o.id, "type": o.ty.to_string(), "formats": o.formats}))
        .collect();
    json!({
        "name": contract.tool.id,
        "title": text(&contract.tool.name, lang).unwrap_or(&contract.tool.id),
        "description": text(&contract.tool.tagline, lang).unwrap_or_default(),
        "input_schema": {
            "$schema": DRAFT,
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
        "x-kortex": {
            "version": contract.tool.version,
            "outputs": outputs,
            "permissions": {
                "network": contract.permissions.network,
                "filesystem": contract.permissions.filesystem,
                "clipboard": contract.permissions.clipboard,
                "sensitive": contract.permissions.sensitive,
                "privacy": contract.permissions.privacy,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> ToolContract {
        crate::parse(
            r##"
schema = 1
[tool]
id = "text.demo"
version = "1.2.0"
universe = "text"
status = "draft"
ui = "auto"
platforms = ["web"]
name = { fr = "Démo", en = "Demo" }
tagline = { fr = "Une démo.", en = "A demo." }
slug = { fr = "demo", en = "demo" }
[[inputs]]
id = "text"
type = "text"
required = true
max_len = 50
label = { fr = "Texte", en = "Text" }
[[inputs]]
id = "image"
type = "file<image/*>"
[[params]]
id = "size"
type = "integer"
default = 3
range = [1.0, 9.0]
level = "medium"
label = { fr = "Taille", en = "Size" }
[[params]]
id = "mode"
type = "enum"
options = ["a", "b"]
default = "a"
level = "pro"
[[params]]
id = "tint"
type = "color"
default = "#0B0F19"
level = "pro"
[[outputs]]
id = "out"
type = "text"
[permissions]
network = false
filesystem = "none"
clipboard = "write"
sensitive = true
privacy = "local"
"##,
        )
        .unwrap_or_else(|e| unreachable!("{e}"))
    }

    #[test]
    fn definition_carries_name_localized_description_and_required_inputs() {
        let fr = tool_definition(&contract(), "fr");
        assert_eq!(fr["name"], "text.demo");
        assert_eq!(fr["title"], "Démo");
        assert_eq!(fr["description"], "Une démo.");
        assert_eq!(fr["input_schema"]["required"], json!(["text"]));
        assert_eq!(fr["input_schema"]["additionalProperties"], false);
        assert_eq!(tool_definition(&contract(), "de")["title"], "Demo");
    }

    #[test]
    fn constraints_come_from_the_contract() {
        let def = tool_definition(&contract(), "en");
        let props = &def["input_schema"]["properties"];
        assert_eq!(props["text"]["maxLength"], 50);
        assert_eq!(props["text"]["description"], "Text");
        assert_eq!(props["size"]["type"], "integer");
        assert_eq!(props["size"]["minimum"], json!(1.0));
        assert_eq!(props["size"]["maximum"], json!(9.0));
        assert_eq!(props["size"]["default"], 3);
        assert_eq!(props["mode"]["enum"], json!(["a", "b"]));
        assert_eq!(props["tint"]["pattern"], "^#[0-9a-fA-F]{6}$");
    }

    #[test]
    fn files_are_references_never_content() {
        let def = tool_definition(&contract(), "en");
        let image = &def["input_schema"]["properties"]["image"];
        assert_eq!(image["type"], "string");
        assert_eq!(image["x-kortex-file-mime"], "image/*");
        let batch = port_schema(&PortType::Files("application/pdf".into()));
        assert_eq!(batch["items"]["x-kortex-file-mime"], "application/pdf");
    }

    #[test]
    fn secrets_are_write_only_and_permissions_are_exposed() {
        assert_eq!(port_schema(&PortType::Secret)["writeOnly"], true);
        let def = tool_definition(&contract(), "en");
        assert_eq!(def["x-kortex"]["permissions"]["sensitive"], true);
        assert_eq!(def["x-kortex"]["permissions"]["clipboard"], "write");
        assert_eq!(def["x-kortex"]["outputs"][0]["type"], "text");
    }

    #[test]
    fn every_port_type_has_a_schema() {
        for ty in [
            "text",
            "rich_text",
            "number",
            "integer",
            "bool",
            "enum",
            "color",
            "date",
            "datetime",
            "duration",
            "range",
            "json",
            "table",
            "secret",
            "url",
            "file<*/*>",
            "files<*/*>",
        ] {
            let parsed: PortType = ty.parse().unwrap_or_else(|e| unreachable!("{e}"));
            assert!(port_schema(&parsed).is_object(), "{ty}");
        }
    }
}
