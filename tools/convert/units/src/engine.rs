use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;

    if !params.value.is_finite() {
        return Err(ToolError::new("invalid_param"));
    }
    fn unit(id: &str) -> Option<(u8, f64, f64)> {
        Some(match id {
            "mm" => (0, 0.001, 0.0),
            "cm" => (0, 0.01, 0.0),
            "m" => (0, 1.0, 0.0),
            "km" => (0, 1000.0, 0.0),
            "in" => (0, 0.0254, 0.0),
            "ft" => (0, 0.3048, 0.0),
            "yd" => (0, 0.9144, 0.0),
            "mi" => (0, 1609.344, 0.0),
            "mg" => (1, 0.000001, 0.0),
            "g" => (1, 0.001, 0.0),
            "kg" => (1, 1.0, 0.0),
            "t" => (1, 1000.0, 0.0),
            "oz" => (1, 0.028349523125, 0.0),
            "lb" => (1, 0.45359237, 0.0),
            "ms" => (2, 0.001, 0.0),
            "s" => (2, 1.0, 0.0),
            "min" => (2, 60.0, 0.0),
            "h" => (2, 3600.0, 0.0),
            "day" => (2, 86400.0, 0.0),
            "ml" => (3, 0.001, 0.0),
            "l" => (3, 1.0, 0.0),
            "us_gal" => (3, 3.785411784, 0.0),
            "m2" => (4, 1.0, 0.0),
            "km2" => (4, 1e6, 0.0),
            "ft2" => (4, 0.09290304, 0.0),
            "ha" => (4, 10000.0, 0.0),
            "c" => (5, 1.0, 273.15),
            "f" => (5, 5.0 / 9.0, 273.15 - 32.0 * 5.0 / 9.0),
            "k" => (5, 1.0, 0.0),
            "b" => (6, 1.0, 0.0),
            "kb" => (6, 1000.0, 0.0),
            "mb" => (6, 1e6, 0.0),
            "gb" => (6, 1e9, 0.0),
            "kib" => (6, 1024.0, 0.0),
            "mib" => (6, 1048576.0, 0.0),
            "gib" => (6, 1073741824.0, 0.0),
            _ => return None,
        })
    }
    let (group, factor, offset) =
        unit(&params.from_unit).ok_or_else(|| ToolError::new("invalid_param"))?;
    let (target, divisor, bias) =
        unit(&params.to_unit).ok_or_else(|| ToolError::new("invalid_param"))?;
    if group != target {
        return Err(ToolError::new("incompatible_units"));
    }
    let base = params.value * factor + offset;
    if group == 5 && base < -1e-10 {
        return Err(ToolError::new("below_absolute_zero"));
    }
    let result = (base - bias) / divisor;
    if !result.is_finite() {
        return Err(ToolError::new("invalid_param"));
    }
    Ok(Outputs {
        result,
        exact: result.to_string(),
    })
}
