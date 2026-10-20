use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;

    if input.text.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let text = match params.direction.as_str() {
        "encode" => {
            let mut out = String::new();
            for c in input.text.chars() {
                match c {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '"' => out.push_str("&quot;"),
                    '\'' => out.push_str("&#39;"),
                    _ => out.push(c),
                }
            }
            out
        }
        "decode" => {
            let mut out = String::new();
            let mut rest = input.text.as_str();
            while let Some(index) = rest.find('&') {
                out.push_str(&rest[..index]);
                rest = &rest[index..];
                let Some(end) = rest.as_bytes().iter().take(33).position(|b| *b == b';') else {
                    out.push('&');
                    rest = &rest[1..];
                    continue;
                };
                let entity = &rest[1..end];
                let decoded = match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some('\u{00a0}'),
                    _ if entity.starts_with('#') => {
                        let n = if let Some(hex) = entity
                            .strip_prefix("#x")
                            .or_else(|| entity.strip_prefix("#X"))
                        {
                            if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                                u32::from_str_radix(hex, 16).ok()
                            } else {
                                None
                            }
                        } else {
                            if entity.len() > 1 && entity[1..].bytes().all(|b| b.is_ascii_digit()) {
                                entity[1..].parse::<u32>().ok()
                            } else {
                                None
                            }
                        };
                        Some(
                            n.filter(|v| *v != 0)
                                .and_then(char::from_u32)
                                .ok_or_else(|| ToolError::new("invalid_param"))?,
                        )
                    }
                    _ => None,
                };
                if let Some(c) = decoded {
                    out.push(c);
                } else {
                    out.push_str(&rest[..=end]);
                }
                rest = &rest[end + 1..];
            }
            out.push_str(rest);
            out
        }
        _ => return Err(ToolError::new("invalid_param")),
    };
    Ok(Outputs { text })
}
