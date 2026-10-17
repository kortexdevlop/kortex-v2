use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if ![params.principal, params.monthly, params.annual_rate]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(ToolError::new("invalid_param"));
    }
    let rate = params.annual_rate / 1200.0;
    let mut final_value = params.principal;
    for _ in 0..params.years * 12 {
        cx.check_cancelled()?;
        final_value = final_value * (1.0 + rate) + params.monthly;
    }
    let contributed = params.principal + params.monthly * (params.years * 12) as f64;
    if !final_value.is_finite() {
        return Err(ToolError::new("invalid_param"));
    }
    Ok(Outputs {
        final_value,
        contributed,
        interest: final_value - contributed,
    })
}
