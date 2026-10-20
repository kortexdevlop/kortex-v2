use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;

    if input.text.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let normalized = input.text.replace("\r\n", "\n").replace('\r', "\n");
    let mut cleaned = String::new();
    for c in normalized.chars() {
        if params.remove_invisible
            && matches!(
                c,
                '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}'
            )
        {
            continue;
        }
        cleaned.push(if c == '\u{00a0}' || c == '\u{202f}' || c == '\t' {
            ' '
        } else {
            c
        });
    }
    let text = cleaned
        .lines()
        .filter_map(|line| {
            let value = if params.collapse_spaces {
                line.split_whitespace().collect::<Vec<_>>().join(" ")
            } else {
                line.trim().to_owned()
            };
            if params.remove_empty && value.is_empty() {
                None
            } else {
                Some(value)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Outputs { text })
}
