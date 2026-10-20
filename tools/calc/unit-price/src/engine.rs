use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if ![
        params.price_a,
        params.quantity_a,
        params.price_b,
        params.quantity_b,
    ]
    .iter()
    .all(|v| v.is_finite())
    {
        return Err(ToolError::new("invalid_param"));
    }
    let unit_a = params.price_a / params.quantity_a * 1000.0;
    let unit_b = params.price_b / params.quantity_b * 1000.0;
    Ok(Outputs {
        unit_a,
        unit_b,
        difference: unit_a - unit_b,
    })
}
