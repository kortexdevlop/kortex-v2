use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

/// Longest result, in bytes: a repetition must not turn into a memory problem.
const MAX_OUTPUT: usize = 1_000_000;

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.text.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let count = usize::try_from(params.count).map_err(|_| ToolError::new("invalid_param"))?;
    let unit = input.text.len().saturating_add(params.separator.len());
    if unit.saturating_mul(count) > MAX_OUTPUT {
        return Err(ToolError::new("too_large"));
    }
    let text = vec![input.text.as_str(); count].join(&params.separator);
    cx.progress(1.0);
    Ok(Outputs { text })
}
