use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;

    if input.text.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut lines = Vec::new();
    let mut removed = 0;
    for line in input.text.lines() {
        cx.check_cancelled()?;
        let key = if params.trim { line.trim() } else { line };
        if params.remove_empty && key.is_empty() {
            removed += 1;
            continue;
        }
        let key = if params.case_sensitive {
            key.to_owned()
        } else {
            key.to_lowercase()
        };
        if seen.insert(key) {
            lines.push(line);
        } else {
            removed += 1;
        }
    }
    Ok(Outputs {
        text: lines.join("\n"),
        removed,
    })
}
