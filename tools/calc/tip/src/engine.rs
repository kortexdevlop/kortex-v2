use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if !params.amount.is_finite() || !params.percent.is_finite() {
        return Err(ToolError::new("invalid_param"));
    }
    let tip = params.amount * params.percent / 100.0;
    let total = params.amount + tip;
    Ok(Outputs {
        tip,
        total,
        per_person: total / params.people as f64,
    })
}
