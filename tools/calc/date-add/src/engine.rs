use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let mut date =
        kortex_date::Date::parse(&params.date).ok_or_else(|| ToolError::new("invalid_param"))?;
    if params.weekdays_only {
        let step = params.days.signum();
        for _ in 0..params.days.unsigned_abs() {
            loop {
                cx.check_cancelled()?;
                date = date
                    .add_days(step)
                    .ok_or_else(|| ToolError::new("invalid_param"))?;
                if date.weekday() < 5 {
                    break;
                }
            }
        }
    } else {
        date = date
            .add_days(params.days)
            .ok_or_else(|| ToolError::new("invalid_param"))?;
    }
    Ok(Outputs { date: date.iso() })
}
