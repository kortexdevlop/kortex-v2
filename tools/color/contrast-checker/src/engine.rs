use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};

use kortex_color::Color;
fn status(ratio: f64, threshold: f64) -> String {
    if ratio >= threshold { "pass" } else { "fail" }.into()
}
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let a = Color::parse("hex", &params.foreground).map_err(|_| ToolError::new("invalid_color"))?;
    let b = Color::parse("hex", &params.background).map_err(|_| ToolError::new("invalid_color"))?;
    let r = a.contrast(b);
    Ok(Outputs {
        ratio: format!("{r:.6}:1"),
        aa_normal: status(r, 4.5),
        aa_large: status(r, 3.0),
        aaa_normal: status(r, 7.0),
        aaa_large: status(r, 4.5),
    })
}
