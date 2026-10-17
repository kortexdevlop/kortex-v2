use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.text.trim().is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let delimiter = match params.delimiter.as_str() {
        "tab" => '\t',
        "semicolon" => ';',
        "comma" => ',',
        _ => return Err(ToolError::new("invalid_param")),
    };
    let mut rows = input
        .text
        .lines()
        .map(|line| {
            line.split(delimiter)
                .map(|cell| {
                    cell.trim()
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;")
                        .replace('\\', "\\\\")
                        .replace('|', "\\|")
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let cols = rows[0].len();
    if cols > 100 || rows.len() > 5000 || rows.iter().any(|r| r.len() != cols) {
        return Err(ToolError::new("invalid_param"));
    }
    let heading = if params.header {
        rows.remove(0)
    } else {
        (1..=cols).map(|i| i.to_string()).collect()
    };
    let marker = match params.alignment.as_str() {
        "left" => ":---",
        "center" => ":---:",
        "right" => "---:",
        _ => return Err(ToolError::new("invalid_param")),
    };
    let render = |row: Vec<String>| format!("| {} |", row.join(" | "));
    let mut result = vec![render(heading), render(vec![marker.into(); cols])];
    result.extend(rows.into_iter().map(render));
    Ok(Outputs {
        text: result.join("\n"),
    })
}
