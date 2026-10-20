use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};

#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if !params.amount.is_finite() || !params.rate.is_finite() {
        return Err(ToolError::new("invalid_param"));
    }
    let multiplier = 1.0 + params.rate / 100.0;
    let (net, gross) = match params.direction.as_str() {
        "net" => (params.amount, params.amount * multiplier),
        "gross" => (params.amount / multiplier, params.amount),
        _ => return Err(ToolError::new("invalid_param")),
    };
    Ok(Outputs {
        net,
        tax: gross - net,
        gross,
    })
}
