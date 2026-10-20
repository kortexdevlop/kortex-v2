use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;

    let source = input.text.trim();
    if source.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let base = u32::try_from(params.from_base).map_err(|_| ToolError::new("invalid_param"))?;
    let value = i128::from_str_radix(source, base).map_err(|_| ToolError::new("invalid_param"))?;
    let target = u128::try_from(params.to_base).map_err(|_| ToolError::new("invalid_param"))?;
    let alphabet = if params.uppercase {
        b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    } else {
        b"0123456789abcdefghijklmnopqrstuvwxyz"
    };
    let mut remaining = value.unsigned_abs();
    let mut digits = Vec::new();
    loop {
        let index =
            usize::try_from(remaining % target).map_err(|_| ToolError::new("invalid_param"))?;
        digits.push(char::from(alphabet[index]));
        remaining /= target;
        if remaining == 0 {
            break;
        }
    }
    if value < 0 {
        digits.push('-');
    }
    digits.reverse();
    Ok(Outputs {
        text: digits.into_iter().collect(),
    })
}
