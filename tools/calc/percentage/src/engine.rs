use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};

#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let (a, b) = (params.a, params.b);
    if !a.is_finite() || !b.is_finite() {
        return Err(ToolError::new("invalid_param"));
    }
    let result = match params.operation.as_str() {
        "part" => a * b / 100.0,
        "share" if b != 0.0 => a / b * 100.0,
        "change" if a != 0.0 => (b - a) / a * 100.0,
        "share" | "change" => return Err(ToolError::new("division_by_zero")),
        "increase" => a * (1.0 + b / 100.0),
        "decrease" => a * (1.0 - b / 100.0),
        _ => return Err(ToolError::new("invalid_param")),
    };
    if !result.is_finite() {
        return Err(ToolError::new("invalid_param"));
    }
    Ok(Outputs { result })
}
