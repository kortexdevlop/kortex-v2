//! Tool definitions for the model, derived from contracts (never written by hand).
use kortex_contract::{PortType, ToolContract, tool_definition};
use serde_json::{Value, json};

/// Function names accept letters, digits, `_` and `-`: `text.cleaner` becomes `text__cleaner`.
#[must_use]
pub fn function_name(tool_id: &str) -> String {
    tool_id.replace('.', "__")
}

#[must_use]
pub fn tool_id(function_name: &str) -> String {
    function_name.replacen("__", ".", 1)
}

fn reference_hint(lang: &str) -> &'static str {
    if lang == "fr" {
        " — référence : @texteN, @fichierN ou @cN.sortie"
    } else {
        " — reference: @texteN, @fichierN or @cN.output"
    }
}

/// OpenAI-style tool object for one contract. Data inputs are described as references, and the
/// contract's own annotations (`x-kortex-*`, `$schema`) are removed to save the model's tokens.
#[must_use]
pub fn definition(contract: &ToolContract, lang: &str) -> Value {
    let full = tool_definition(contract, lang);
    let mut parameters = full["input_schema"].clone();
    if let Some(object) = parameters.as_object_mut() {
        object.remove("$schema");
    }
    if let Some(properties) = parameters
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    {
        for input in &contract.inputs {
            let is_data = matches!(
                input.ty,
                PortType::Text | PortType::RichText | PortType::File(_) | PortType::Files(_)
            );
            if let (true, Some(property)) = (
                is_data,
                properties.get_mut(&input.id).and_then(Value::as_object_mut),
            ) {
                let label = property
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(&input.id)
                    .to_owned();
                property.insert(
                    "description".into(),
                    json!(format!("{label}{}", reference_hint(lang))),
                );
            }
        }
        for property in properties.values_mut().filter_map(Value::as_object_mut) {
            property.retain(|key, _| !key.starts_with("x-kortex-"));
        }
    }
    json!({
        "type": "function",
        "function": {
            "name": function_name(&contract.tool.id),
            "description": full["description"],
            "parameters": parameters,
        }
    })
}
