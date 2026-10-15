//! Password generation: uniform sampling (rejection) from the selected character classes,
//! guaranteeing at least one character of each selected class.
use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.?/";
const AMBIGUOUS: &str = "0O1lI|";

fn classes(p: &Params) -> Vec<Vec<char>> {
    [
        (p.lowercase, LOWER),
        (p.uppercase, UPPER),
        (p.digits, DIGITS),
        (p.symbols, SYMBOLS),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, set)| {
        set.chars()
            .filter(|c| !(p.exclude_ambiguous && AMBIGUOUS.contains(*c)))
            .collect()
    })
    .collect()
}

/// Uniform index in `0..n` by rejection sampling over random bytes.
fn pick(cx: &mut Cx, n: usize) -> Result<usize, ToolError> {
    let limit = 256 - (256 % n.min(256));
    loop {
        let mut b = [0_u8; 1];
        cx.fill_random(&mut b)?;
        if usize::from(b[0]) < limit {
            return Ok(usize::from(b[0]) % n);
        }
    }
}

#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    let classes = classes(&params);
    if classes.is_empty() {
        return Err(ToolError::new("no_character_class"));
    }
    let length =
        usize::try_from(params.length).map_err(|_| ToolError::new(ToolError::INVALID_PARAM))?;
    if length < classes.len() {
        return Err(ToolError::new("length_too_short"));
    }
    let all: Vec<char> = classes.iter().flatten().copied().collect();
    let mut out: Vec<char> = Vec::with_capacity(length);
    for class in &classes {
        out.push(class[pick(cx, class.len())?]);
    }
    while out.len() < length {
        cx.check_cancelled()?;
        out.push(all[pick(cx, all.len())?]);
    }
    // Fisher-Yates shuffle so the guaranteed characters are not in fixed positions.
    for i in (1..out.len()).rev() {
        out.swap(i, pick(cx, i + 1)?);
    }
    cx.progress(1.0);
    Ok(Outputs {
        password: out.into_iter().collect(),
    })
}
