use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if ![params.a, params.b, params.c].iter().all(|v| v.is_finite()) {
        return Err(ToolError::new("invalid_param"));
    }
    if params.a == 0.0 {
        return Err(ToolError::new("division_by_zero"));
    }
    let result = params.b / params.a * params.c;
    if !result.is_finite() {
        return Err(ToolError::new("invalid_param"));
    }
    Ok(Outputs { result })
}
