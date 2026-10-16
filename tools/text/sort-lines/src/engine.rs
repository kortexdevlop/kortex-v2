use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

fn key(s: &str, ignore_case: bool) -> String {
    if ignore_case {
        s.to_lowercase()
    } else {
        s.to_owned()
    }
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.text.trim().is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    let original = input.text.lines().count();
    let mut lines: Vec<String> = input
        .text
        .lines()
        .map(|l| {
            if params.trim {
                l.trim().to_owned()
            } else {
                l.to_owned()
            }
        })
        .collect();
    if params.remove_empty {
        lines.retain(|l| !l.trim().is_empty());
    }
    if params.dedupe {
        let mut seen = std::collections::HashSet::new();
        lines.retain(|l| seen.insert(key(l, params.ignore_case)));
    }
    match params.sort.as_str() {
        "none" => {}
        "asc" => lines.sort_by_cached_key(|l| key(l, params.ignore_case)),
        "desc" => {
            lines.sort_by_cached_key(|l| key(l, params.ignore_case));
            lines.reverse();
        }
        "length" => lines.sort_by_key(|l| l.chars().count()),
        "reverse" => lines.reverse(),
        _ => return Err(ToolError::with_detail(ToolError::INVALID_PARAM, "sort")),
    }
    let kept = lines.len();
    Ok(Outputs {
        text: lines.join("\n"),
        lines: count(kept),
        removed: count(original.saturating_sub(kept)),
    })
}
