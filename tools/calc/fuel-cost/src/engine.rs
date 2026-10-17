use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if ![
        params.distance,
        params.consumption,
        params.price,
        params.extra,
    ]
    .iter()
    .all(|v| v.is_finite())
    {
        return Err(ToolError::new("invalid_param"));
    }
    let distance = params.distance * if params.return_trip { 2.0 } else { 1.0 };
    let litres = distance * params.consumption / 100.0;
    let total = litres * params.price + params.extra;
    Ok(Outputs {
        litres,
        total,
        per_person: total / params.people as f64,
    })
}
