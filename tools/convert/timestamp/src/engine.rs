use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let text = input.text.trim();
    if text.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let text = match params.direction.as_str() {
        "to_date" => {
            let value = text
                .parse::<i64>()
                .map_err(|_| ToolError::new("invalid_param"))?;
            let millis = match params.unit.as_str() {
                "seconds" => value
                    .checked_mul(1000)
                    .ok_or_else(|| ToolError::new("invalid_param"))?,
                "milliseconds" => value,
                _ => return Err(ToolError::new("invalid_param")),
            };
            kortex_date::format_utc(millis).ok_or_else(|| ToolError::new("invalid_param"))?
        }
        "to_timestamp" => {
            let millis =
                kortex_date::parse_utc(text).ok_or_else(|| ToolError::new("invalid_param"))?;
            match params.unit.as_str() {
                "seconds" => millis.div_euclid(1000).to_string(),
                "milliseconds" => millis.to_string(),
                _ => return Err(ToolError::new("invalid_param")),
            }
        }
        _ => return Err(ToolError::new("invalid_param")),
    };
    Ok(Outputs { text })
}
