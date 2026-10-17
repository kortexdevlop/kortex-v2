use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let birth =
        kortex_date::Date::parse(&params.birth).ok_or_else(|| ToolError::new("invalid_param"))?;
    let at = kortex_date::Date::parse(&params.at).ok_or_else(|| ToolError::new("invalid_param"))?;
    if at < birth {
        return Err(ToolError::new("invalid_param"));
    }
    let anniversary = (
        birth.month,
        birth.day.min(kortex_date::month_days(at.year, birth.month)),
    );
    let years = at.year - birth.year - i64::from((at.month, at.day) < anniversary);
    Ok(Outputs {
        years,
        days: at.ordinal() - birth.ordinal(),
    })
}
