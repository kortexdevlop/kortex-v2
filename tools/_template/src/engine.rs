//! Pure engine: no UI, no direct platform access.
use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

/// Placeholder behaviour: returns the input unchanged.
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.text.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    cx.progress(1.0);
    Ok(Outputs { text: input.text })
}
