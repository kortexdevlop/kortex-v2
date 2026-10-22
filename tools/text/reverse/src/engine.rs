use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

/// Letters in reverse order, by Unicode scalar value (combining marks stay after their base only
/// when they are separate scalars, which is the usual text-tool behaviour).
fn characters(text: &str) -> String {
    text.chars().rev().collect()
}

/// Words in reverse order, each line on its own: spacing between words is normalised to one space.
fn words(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            let mut parts: Vec<&str> = line.split_whitespace().collect();
            parts.reverse();
            parts.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Lines in reverse order (a final newline stays final).
fn lines(text: &str) -> String {
    let trailing = text.ends_with('\n');
    let body = text.strip_suffix('\n').unwrap_or(text);
    let mut parts: Vec<&str> = body.split('\n').collect();
    parts.reverse();
    let mut out = parts.join("\n");
    if trailing {
        out.push('\n');
    }
    out
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.text.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let text = match params.mode.as_str() {
        "characters" => characters(&input.text),
        "words" => words(&input.text),
        "lines" => lines(&input.text),
        _ => return Err(ToolError::new("invalid_param")),
    };
    cx.progress(1.0);
    Ok(Outputs { text })
}
