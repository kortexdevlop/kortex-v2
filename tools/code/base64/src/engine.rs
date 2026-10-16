use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

const STD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn encode(bytes: &[u8], url_safe: bool, padding: bool) -> String {
    let table = if url_safe { URL } else { STD };
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |a, (i, b)| a | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(char::from(table[((n >> (18 - 6 * i)) & 63) as usize]));
        }
        if padding {
            for _ in chunk.len()..3 {
                out.push('=');
            }
        }
    }
    out
}

/// Accepts both alphabets, ignores whitespace, and tolerates missing padding.
fn decode(s: &str) -> Option<Vec<u8>> {
    let mut vals = Vec::new();
    let mut padding_seen = false;
    for c in s.chars().filter(|c| !c.is_whitespace()) {
        if c == '=' {
            padding_seen = true;
            continue;
        }
        if padding_seen {
            return None; // data after padding
        }
        let v = match c {
            'A'..='Z' => c as u8 - b'A',
            'a'..='z' => c as u8 - b'a' + 26,
            '0'..='9' => c as u8 - b'0' + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            _ => return None,
        };
        vals.push(v);
    }
    if vals.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(vals.len() * 3 / 4);
    for chunk in vals.chunks(4) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |a, (i, v)| a | (u32::from(*v) << (18 - 6 * i)));
        for i in 0..chunk.len() - 1 {
            out.push(((n >> (16 - 8 * i)) & 0xFF) as u8);
        }
    }
    Some(out)
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.text.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let text = match params.direction.as_str() {
        "encode" => encode(input.text.as_bytes(), params.url_safe, params.padding),
        "decode" => {
            let bytes = decode(&input.text).ok_or_else(|| ToolError::new("invalid_base64"))?;
            String::from_utf8(bytes).map_err(|_| ToolError::new("not_text"))?
        }
        _ => {
            return Err(ToolError::with_detail(
                ToolError::INVALID_PARAM,
                "direction",
            ));
        }
    };
    Ok(Outputs { text })
}
