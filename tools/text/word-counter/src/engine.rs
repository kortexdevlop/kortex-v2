use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

/// Sentences end with `.`, `!`, `?` or `…` followed by whitespace or the end, and must contain a
/// letter or digit (so `3.14` and `...` alone are not sentences).
fn sentences(text: &str) -> i64 {
    let chars: Vec<char> = text.chars().collect();
    let (mut count, mut has_word, mut i) = (0, false, 0);
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphanumeric() {
            has_word = true;
        }
        if matches!(c, '.' | '!' | '?' | '…') {
            let mut j = i + 1;
            while j < chars.len()
                && matches!(
                    chars[j],
                    '.' | '!' | '?' | '…' | '"' | '»' | '”' | '\'' | ')'
                )
            {
                j += 1;
            }
            if (j >= chars.len() || chars[j].is_whitespace()) && has_word {
                count += 1;
                has_word = false;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    count + i64::from(has_word)
}

fn paragraphs(text: &str) -> i64 {
    let (mut count, mut in_block) = (0, false);
    for line in text.lines() {
        if line.trim().is_empty() {
            in_block = false;
        } else if !in_block {
            in_block = true;
            count += 1;
        }
    }
    count
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let t = &input.text;
    let words = count(t.split_whitespace().count());
    #[allow(clippy::cast_precision_loss)]
    let minutes = (words as f64 / params.wpm as f64 * 10.0).round() / 10.0;
    Ok(Outputs {
        words,
        characters: count(t.chars().count()),
        characters_no_spaces: count(t.chars().filter(|c| !c.is_whitespace()).count()),
        sentences: sentences(t),
        paragraphs: paragraphs(t),
        lines: count(t.lines().count()),
        reading_minutes: minutes,
    })
}
