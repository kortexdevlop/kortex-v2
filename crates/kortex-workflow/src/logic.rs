//! Built-in nodes that are not tools: a user-picked file, and the small set of text logic nodes
//! (condition, filter, merge, delay). Their ports are fixed here, not in a tool contract, so the
//! same validation and execution rules apply to them as to any tool.
use kortex_contract::PortType;
use kortex_runtime::{Value, ValueMap};

use crate::{ParamSpec, Port, RunError, TEXT_PORT, ToolPorts};

pub const FILE_PORT: &str = "file";
pub const FILES_PORT: &str = "files";
/// Pseudo tool name of the delay job: the host sleeps, then returns the text unchanged.
pub const DELAY_TOOL: &str = "flow.delay";
/// Longest delay a node may ask for.
pub const MAX_DELAY_MS: i64 = 30_000;

pub const LOGIC_OPS: [&str; 4] = ["condition", "filter", "merge", "delay"];
pub const TESTS: [&str; 7] = [
    "contains",
    "not_contains",
    "equals",
    "starts_with",
    "ends_with",
    "is_empty",
    "not_empty",
];

fn input(id: &str, required: bool) -> Port {
    Port {
        id: id.to_owned(),
        ty: PortType::Text,
        required,
    }
}

fn param(id: &str, ty: PortType, default: &str, options: &[&str]) -> ParamSpec {
    ParamSpec {
        id: id.to_owned(),
        ty,
        default: default.to_owned(),
        options: options.iter().map(|o| (*o).to_owned()).collect(),
    }
}

/// Ports and params of a logic node, `None` for an unknown operation.
#[must_use]
pub fn logic_ports(op: &str) -> Option<ToolPorts> {
    let test = || param("test", PortType::Enum, "contains", &TESTS);
    let value = || param("value", PortType::Text, "", &[]);
    let case = || param("ignore_case", PortType::Bool, "false", &[]);
    Some(match op {
        "condition" => ToolPorts {
            inputs: vec![input(TEXT_PORT, true)],
            params: vec![test(), value(), case()],
            outputs: vec![input("then", false), input("else", false)],
        },
        "filter" => ToolPorts {
            inputs: vec![input(TEXT_PORT, true)],
            params: vec![
                param("keep", PortType::Enum, "keep", &["keep", "drop"]),
                test(),
                value(),
                case(),
            ],
            outputs: vec![input(TEXT_PORT, false)],
        },
        "merge" => ToolPorts {
            inputs: vec![input("a", false), input("b", false), input("c", false)],
            params: vec![param("separator", PortType::Text, "\n", &[])],
            outputs: vec![input(TEXT_PORT, false)],
        },
        "delay" => ToolPorts {
            inputs: vec![input(TEXT_PORT, true)],
            params: vec![param("ms", PortType::Integer, "1000", &[])],
            outputs: vec![input(TEXT_PORT, false)],
        },
        _ => return None,
    })
}

/// Output port of a file node: a single file or a batch, typed by what it accepts.
#[must_use]
pub fn file_ports(accept: &str, multiple: bool) -> ToolPorts {
    let accept = if accept.is_empty() { "*/*" } else { accept };
    ToolPorts {
        inputs: Vec::new(),
        params: Vec::new(),
        outputs: vec![Port {
            id: if multiple { FILES_PORT } else { FILE_PORT }.to_owned(),
            ty: if multiple {
                PortType::Files(accept.to_owned())
            } else {
                PortType::File(accept.to_owned())
            },
            required: false,
        }],
    }
}

/// Whether `text` passes `test` against `value`.
#[must_use]
pub fn passes(text: &str, test: &str, value: &str, ignore_case: bool) -> bool {
    let (text, value) = if ignore_case {
        (text.to_lowercase(), value.to_lowercase())
    } else {
        (text.to_owned(), value.to_owned())
    };
    match test {
        "contains" => text.contains(&value),
        "not_contains" => !text.contains(&value),
        "equals" => text == value,
        "starts_with" => text.starts_with(&value),
        "ends_with" => text.ends_with(&value),
        "is_empty" => text.trim().is_empty(),
        "not_empty" => !text.trim().is_empty(),
        _ => false,
    }
}

fn text_of(map: &ValueMap, key: &str) -> Option<String> {
    match map.get(key) {
        Some(Value::Text(t)) => Some(t.clone()),
        _ => None,
    }
}

/// Runs a logic node that needs no host (everything but the delay). A condition produces only the
/// port of the branch taken; the other branch's nodes are skipped by the executor.
///
/// # Errors
/// `missing_input` or `unknown_logic`.
pub fn eval(op: &str, inputs: &ValueMap, params: &ValueMap) -> Result<ValueMap, RunError> {
    let p = |k: &str| text_of(params, k).unwrap_or_default();
    let flag = matches!(params.get("ignore_case"), Some(Value::Bool(true)));
    let text = || {
        text_of(inputs, TEXT_PORT).ok_or_else(|| RunError {
            code: "missing_input".into(),
            detail: Some(TEXT_PORT.into()),
        })
    };
    match op {
        "condition" => {
            let text = text()?;
            let port = if passes(&text, &p("test"), &p("value"), flag) {
                "then"
            } else {
                "else"
            };
            Ok(ValueMap::from([(port.to_owned(), Value::Text(text))]))
        }
        "filter" => {
            let keep = p("keep") != "drop";
            let lines: Vec<String> = text()?
                .lines()
                .filter(|line| passes(line, &p("test"), &p("value"), flag) == keep)
                .map(str::to_owned)
                .collect();
            Ok(ValueMap::from([(
                TEXT_PORT.to_owned(),
                Value::Text(lines.join("\n")),
            )]))
        }
        "merge" => {
            let parts: Vec<String> = ["a", "b", "c"]
                .iter()
                .filter_map(|k| text_of(inputs, k))
                .collect();
            Ok(ValueMap::from([(
                TEXT_PORT.to_owned(),
                Value::Text(parts.join(&p("separator"))),
            )]))
        }
        other => Err(RunError {
            code: "unknown_logic".into(),
            detail: Some(other.to_owned()),
        }),
    }
}
