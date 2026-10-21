//! Body mass index (adults): weight / height², with the WHO category and the weight range that
//! gives a "normal" index (18.5 to 24.9) for that height. A screening figure, not a diagnosis.
use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

const NORMAL_MIN: f64 = 18.5;
const NORMAL_MAX: f64 = 24.9;

/// WHO adult categories (codes are translated by the host).
fn category(bmi: f64) -> &'static str {
    match bmi {
        b if b < 16.0 => "severe_thinness",
        b if b < 17.0 => "moderate_thinness",
        b if b < NORMAL_MIN => "mild_thinness",
        b if b < 25.0 => "normal",
        b if b < 30.0 => "overweight",
        b if b < 35.0 => "obesity_1",
        b if b < 40.0 => "obesity_2",
        _ => "obesity_3",
    }
}

fn round(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}

#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if !params.weight_kg.is_finite() || !params.height_cm.is_finite() || params.height_cm <= 0.0 {
        return Err(ToolError::new("invalid_param"));
    }
    let metres = params.height_cm / 100.0;
    let bmi = params.weight_kg / (metres * metres);
    Ok(Outputs {
        bmi: round(bmi, 1),
        category: category(bmi).to_owned(),
        healthy_min_kg: round(NORMAL_MIN * metres * metres, 1),
        healthy_max_kg: round(NORMAL_MAX * metres * metres, 1),
    })
}
