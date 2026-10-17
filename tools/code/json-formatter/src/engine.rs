use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};
use serde::Serialize;
use serde_json::Value;
use serde_json::ser::PrettyFormatter;

fn sort_keys(v: &mut Value) {
    match v {
        Value::Object(m) => {
            let mut entries: Vec<_> = std::mem::take(m).into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, x) in &mut entries {
                sort_keys(x);
            }
            *m = entries.into_iter().collect();
        }
        Value::Array(a) => a.iter_mut().for_each(sort_keys),
        _ => {}
    }
}

fn pretty(v: &Value, indent: usize) -> Result<String, ToolError> {
    let pad = " ".repeat(indent);
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut buf,
        PrettyFormatter::with_indent(pad.as_bytes()),
    );
    v.serialize(&mut ser)
        .map_err(|_| ToolError::new("serialize_failed"))?;
    String::from_utf8(buf).map_err(|_| ToolError::new("serialize_failed"))
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.text.trim().is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let mut value: Value = serde_json::from_str(&input.text).map_err(|e| {
        ToolError::with_detail("invalid_json", format!("{}:{}", e.line(), e.column()))
    })?;
    if params.sort_keys {
        sort_keys(&mut value);
    }
    let result = match params.action.as_str() {
        "format" => pretty(&value, usize::try_from(params.indent).unwrap_or(2))?,
        "minify" => {
            serde_json::to_string(&value).map_err(|_| ToolError::new("serialize_failed"))?
        }
        "validate" => String::new(),
        _ => return Err(ToolError::with_detail(ToolError::INVALID_PARAM, "action")),
    };
    Ok(Outputs {
        result,
        status: "valid".to_owned(),
    })
}
