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

pub const LOGIC_OPS: [&str; 6] = [
    "condition",
    "filter",
    "merge",
    "delay",
    "template",
    "foreach",
];
/// Most lines a "for each line" node accepts, and the most it can be asked to run.
pub const MAX_ITEMS: i64 = 200;
/// Longest text a template may produce.
pub const MAX_TEMPLATE_CHARS: usize = 1_000_000;
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
        "template" => ToolPorts {
            inputs: vec![input("a", false), input("b", false), input("c", false)],
            params: vec![param("template", PortType::Text, "{a}", &[])],
            outputs: vec![input(TEXT_PORT, false)],
        },
        "foreach" => ToolPorts {
            inputs: vec![input(TEXT_PORT, true)],
            params: vec![
                param("tool", PortType::Text, "", &[]),
                param("options", PortType::Text, "", &[]),
                param("max", PortType::Integer, "100", &[]),
            ],
            outputs: vec![input(TEXT_PORT, false)],
        },
        _ => return None,
    })
}

/// The text input and text output a tool exposes to a "for each line" node: its single required
/// input must be text, and it must produce a text.
#[must_use]
pub fn text_io(ports: &ToolPorts) -> Option<(String, String)> {
    let mut required = ports.inputs.iter().filter(|i| i.required);
    let input = required.next().filter(|i| i.ty == PortType::Text)?;
    if required.next().is_some() {
        return None;
    }
    let output = ports.outputs.iter().find(|o| o.ty == PortType::Text)?;
    Some((input.id.clone(), output.id.clone()))
}

/// `key=value` per line, for the parameters of the tool a loop runs. Blank lines are ignored.
#[must_use]
pub fn parse_options(text: &str) -> std::collections::BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

fn apply_filter(value: &str, filter: &str) -> Option<String> {
    Some(match filter {
        "upper" => value.to_uppercase(),
        "lower" => value.to_lowercase(),
        "trim" => value.trim().to_owned(),
        "length" => value.chars().count().to_string(),
        "lines" => value.lines().count().to_string(),
        "first_line" => value.lines().next().unwrap_or("").to_owned(),
        _ => return None,
    })
}

/// Fills `{a}`, `{b}`, `{c}` (optionally `{a|trim|upper}`) in a template; `{{` and `}}` are the
/// braces themselves. Bounded: no loops, no evaluation, output capped.
///
/// # Errors
/// `bad_template` with the offending placeholder, or `too_large`.
pub fn render(template: &str, inputs: &ValueMap) -> Result<String, RunError> {
    let bad = |detail: &str| RunError {
        code: "bad_template".into(),
        detail: Some(detail.to_owned()),
    };
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut body = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(ch) => body.push(ch),
                        None => return Err(bad("{")),
                    }
                }
                let mut parts = body.split('|').map(str::trim);
                let name = parts.next().unwrap_or("");
                if !matches!(name, "a" | "b" | "c") {
                    return Err(bad(&body));
                }
                let mut value = text_of(inputs, name).unwrap_or_default();
                for filter in parts {
                    value = apply_filter(&value, filter).ok_or_else(|| bad(&body))?;
                }
                out.push_str(&value);
            }
            '}' => return Err(bad("}")),
            other => out.push(other),
        }
        if out.chars().count() > MAX_TEMPLATE_CHARS {
            return Err(RunError {
                code: "too_large".into(),
                detail: None,
            });
        }
    }
    Ok(out)
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
        "template" => Ok(ValueMap::from([(
            TEXT_PORT.to_owned(),
            Value::Text(render(&p("template"), inputs)?),
        )])),
        other => Err(RunError {
            code: "unknown_logic".into(),
            detail: Some(other.to_owned()),
        }),
    }
}
