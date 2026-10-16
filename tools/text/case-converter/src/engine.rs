use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

/// Words of an identifier or sentence: split at non-alphanumerics and at lower→upper changes.
fn words(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut prev_lower) = (Vec::new(), String::new(), false);
    for c in s.chars() {
        if c.is_alphanumeric() {
            if c.is_uppercase() && prev_lower && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(c);
            prev_lower = c.is_lowercase() || c.is_numeric();
        } else {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn capitalize(w: &str) -> String {
    let mut cs = w.chars();
    cs.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase()
    })
}

/// Capitalizes the first letter of every word, keeping spaces and punctuation as typed.
fn title(s: &str) -> String {
    let (mut out, mut start) = (String::new(), true);
    for c in s.chars() {
        if c.is_alphanumeric() {
            if start {
                out.extend(c.to_uppercase())
            } else {
                out.extend(c.to_lowercase())
            }
            start = false;
        } else {
            out.push(c);
            // An apostrophe stays inside a word ("don't", "l'été").
            start = !matches!(c, '\'' | '’');
        }
    }
    out
}

/// First letter of each sentence in upper case, the rest in lower case.
fn sentence(s: &str) -> String {
    let (mut out, mut start, mut pending) = (String::new(), true, false);
    for c in s.chars() {
        if start && !c.is_whitespace() {
            // The first visible character opens the sentence, even if it is a digit or a quote.
            if c.is_alphabetic() {
                out.extend(c.to_uppercase())
            } else {
                out.push(c)
            }
            start = false;
        } else {
            out.extend(c.to_lowercase());
        }
        if matches!(c, '.' | '!' | '?' | '…') {
            pending = true;
        } else if c.is_whitespace() && pending {
            start = true;
            pending = false;
        } else if !c.is_whitespace() {
            pending = false;
        }
    }
    out
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let s = &input.text;
    if s.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let w = || words(s);
    let text = match params.case.as_str() {
        "upper" => s.to_uppercase(),
        "lower" => s.to_lowercase(),
        "title" => title(s),
        "sentence" => sentence(s),
        "camel" => w()
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if i == 0 {
                    x.to_lowercase()
                } else {
                    capitalize(x)
                }
            })
            .collect(),
        "pascal" => w().iter().map(|x| capitalize(x)).collect(),
        "snake" => w()
            .iter()
            .map(|x| x.to_lowercase())
            .collect::<Vec<_>>()
            .join("_"),
        "kebab" => w()
            .iter()
            .map(|x| x.to_lowercase())
            .collect::<Vec<_>>()
            .join("-"),
        "constant" => w()
            .iter()
            .map(|x| x.to_uppercase())
            .collect::<Vec<_>>()
            .join("_"),
        _ => return Err(ToolError::with_detail(ToolError::INVALID_PARAM, "case")),
    };
    Ok(Outputs { text })
}
