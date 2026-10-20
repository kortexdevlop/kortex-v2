use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};

use kortex_color::Color;
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let c =
        Color::parse(&params.format, &input.text).map_err(|_| ToolError::new("invalid_color"))?;
    Ok(Outputs {
        hex: c.hex(),
        rgb: c.rgb(),
        hsl: c.hsl(),
        oklch: c.oklch(),
        gamut: if c.in_gamut() { "inside" } else { "clipped" }.into(),
    })
}
