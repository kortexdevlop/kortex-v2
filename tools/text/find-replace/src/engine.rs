use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;

    if params.search.is_empty() {
        return Err(ToolError::new("invalid_param"));
    }
    let count = if params.first_only {
        usize::from(input.text.contains(&params.search))
    } else {
        input.text.matches(&params.search).count()
    };
    let size = input
        .text
        .len()
        .saturating_add(count.saturating_mul(params.replacement.len()));
    if size > 10_000_000 {
        return Err(ToolError::new("invalid_param"));
    }
    let text = if params.first_only {
        input.text.replacen(&params.search, &params.replacement, 1)
    } else {
        input.text.replace(&params.search, &params.replacement)
    };
    Ok(Outputs {
        text,
        count: i64::try_from(count).map_err(|_| ToolError::new("invalid_param"))?,
    })
}
