use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let start =
        kortex_date::Date::parse(&params.start).ok_or_else(|| ToolError::new("invalid_param"))?;
    let end =
        kortex_date::Date::parse(&params.end).ok_or_else(|| ToolError::new("invalid_param"))?;
    let mut days = end.ordinal() - start.ordinal();
    if params.inclusive {
        days += if days < 0 { -1 } else { 1 };
    }
    Ok(Outputs {
        days,
        weeks: days as f64 / 7.0,
    })
}
