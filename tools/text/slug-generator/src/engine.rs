use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};

fn latin(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => "a",
        'ç' | 'ć' | 'č' => "c",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' => "e",
        'ì' | 'í' | 'î' | 'ï' => "i",
        'ñ' | 'ń' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => "o",
        'ù' | 'ú' | 'û' | 'ü' => "u",
        'ý' | 'ÿ' => "y",
        'ß' => "ss",
        'œ' => "oe",
        'æ' => "ae",
        'ł' => "l",
        'š' | 'ś' => "s",
        'ž' | 'ź' | 'ż' => "z",
        _ => return None,
    })
}
#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.text.trim().is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let sep = match params.separator.as_str() {
        "dash" => '-',
        "underscore" => '_',
        _ => return Err(ToolError::new("invalid_param")),
    };
    let mut out = String::new();
    let mut pending = false;
    for c in input.text.to_lowercase().chars() {
        if ('\u{0300}'..='\u{036f}').contains(&c) {
            continue;
        }
        let mapped = if params.ascii_only { latin(c) } else { None };
        if let Some(s) = mapped {
            if pending && !out.is_empty() {
                out.push(sep);
            }
            out.push_str(s);
            pending = false;
        } else if c.is_ascii_alphanumeric() || (!params.ascii_only && c.is_alphanumeric()) {
            if pending && !out.is_empty() {
                out.push(sep);
            }
            out.push(c);
            pending = false;
        } else {
            pending = true;
        }
    }
    let text = out
        .chars()
        .take(usize::try_from(params.max_length).unwrap_or(80))
        .collect::<String>()
        .trim_end_matches(sep)
        .to_owned();
    if text.is_empty() {
        return Err(ToolError::new("empty_slug"));
    }
    Ok(Outputs { text })
}
