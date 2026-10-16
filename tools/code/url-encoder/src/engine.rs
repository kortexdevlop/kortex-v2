use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

fn keep(b: u8, uri: bool) -> bool {
    let unreserved = b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
        );
    unreserved
        || (uri
            && matches!(
                b,
                b';' | b'/' | b'?' | b':' | b'@' | b'&' | b'=' | b'+' | b'$' | b',' | b'#'
            ))
}

fn encode(s: &str, uri: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if keep(b, uri) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    char::from(b).to_digit(16).map(|d| d as u8)
}

fn decode(s: &str, plus_as_space: bool) -> Result<String, ToolError> {
    let bytes = s.as_bytes();
    let (mut out, mut i) = (Vec::with_capacity(bytes.len()), 0);
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let (h, l) = (
                    bytes.get(i + 1).copied().and_then(hex),
                    bytes.get(i + 2).copied().and_then(hex),
                );
                let (Some(h), Some(l)) = (h, l) else {
                    return Err(ToolError::new("invalid_percent"));
                };
                out.push(h * 16 + l);
                i += 3;
            }
            b'+' if plus_as_space => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| ToolError::new("not_text"))
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.text.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let text = match (params.direction.as_str(), params.scope.as_str()) {
        ("encode", "component") => encode(&input.text, false),
        ("encode", "uri") => encode(&input.text, true),
        ("decode", _) => decode(&input.text, params.plus_as_space)?,
        _ => {
            return Err(ToolError::with_detail(
                ToolError::INVALID_PARAM,
                "direction/scope",
            ));
        }
    };
    Ok(Outputs { text })
}
