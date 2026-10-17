use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let source = input.text.trim();
    if source.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let text = match params.direction.as_str() {
        "to_symbolic" => {
            let source = source
                .strip_prefix('0')
                .filter(|s| s.len() == 3)
                .unwrap_or(source);
            if source.len() != 3 || !source.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
                return Err(ToolError::new("invalid_param"));
            }
            let mut out = String::new();
            for b in source.bytes() {
                let n = b - b'0';
                out.push(if n & 4 != 0 { 'r' } else { '-' });
                out.push(if n & 2 != 0 { 'w' } else { '-' });
                out.push(if n & 1 != 0 { 'x' } else { '-' });
            }
            out
        }
        "to_octal" => {
            let b = source.as_bytes();
            if b.len() != 9 {
                return Err(ToolError::new("invalid_param"));
            }
            let mut out = String::new();
            for chunk in b.chunks(3) {
                let mut n = 0;
                for (i, c) in chunk.iter().enumerate() {
                    if *c == b"rwx"[i] {
                        n |= 4 >> i;
                    } else if *c != b'-' {
                        return Err(ToolError::new("invalid_param"));
                    }
                }
                out.push(char::from(b'0' + n));
            }
            out
        }
        _ => return Err(ToolError::new("invalid_param")),
    };
    Ok(Outputs { text })
}
