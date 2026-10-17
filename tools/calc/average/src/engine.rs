use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let mut values = Vec::new();
    for token in input.text.split(|c: char| c.is_whitespace() || c == ';') {
        if token.is_empty() {
            continue;
        }
        let n = token
            .replace(',', ".")
            .parse::<f64>()
            .map_err(|_| ToolError::new("invalid_param"))?;
        if !n.is_finite() {
            return Err(ToolError::new("invalid_param"));
        }
        values.push(n);
    }
    if values.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    values.sort_by(f64::total_cmp);
    let count = i64::try_from(values.len()).map_err(|_| ToolError::new("invalid_param"))?;
    let sum = values.iter().sum::<f64>();
    let mean = values.iter().map(|v| v / count as f64).sum::<f64>();
    let mid = values.len() / 2;
    let median = if values.len() % 2 == 0 {
        values[mid - 1] / 2.0 + values[mid] / 2.0
    } else {
        values[mid]
    };
    if !sum.is_finite() || !mean.is_finite() || !median.is_finite() {
        return Err(ToolError::new("invalid_param"));
    }
    Ok(Outputs {
        count,
        mean,
        median,
        sum,
        min: values[0],
        max: values[values.len() - 1],
    })
}
