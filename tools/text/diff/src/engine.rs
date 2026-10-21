//! Line-by-line text comparison (longest common subsequence). The result lists every line with a
//! `+ ` (added), `- ` (removed) or two-space (unchanged) prefix, plus the three counts.
use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

/// Lines per side: the comparison table is `lines²` cells.
const MAX_LINES: usize = 4000;

fn normalize(line: &str, params: &Params) -> String {
    let mut text = if params.ignore_whitespace {
        line.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        line.to_owned()
    };
    if params.ignore_case {
        text = text.to_lowercase();
    }
    text
}

enum Edit {
    Same(usize),
    Removed(usize),
    Added(usize),
}

/// Edit script from the longest common subsequence of the (normalized) lines.
fn script(a: &[String], b: &[String], cx: &Cx) -> Result<Vec<Edit>, ToolError> {
    let (n, m) = (a.len(), b.len());
    let width = m + 1;
    let mut table = vec![0_u32; (n + 1) * width];
    for i in (0..n).rev() {
        cx.check_cancelled()?;
        for j in (0..m).rev() {
            table[i * width + j] = if a[i] == b[j] {
                table[(i + 1) * width + j + 1] + 1
            } else {
                table[(i + 1) * width + j].max(table[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut edits = Vec::with_capacity(n + m);
    while i < n && j < m {
        if a[i] == b[j] {
            edits.push(Edit::Same(i));
            i += 1;
            j += 1;
        } else if table[(i + 1) * width + j] >= table[i * width + j + 1] {
            edits.push(Edit::Removed(i));
            i += 1;
        } else {
            edits.push(Edit::Added(j));
            j += 1;
        }
    }
    edits.extend((i..n).map(Edit::Removed));
    edits.extend((j..m).map(Edit::Added));
    Ok(edits)
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let original: Vec<&str> = input.original.lines().collect();
    let modified: Vec<&str> = input.modified.lines().collect();
    if original.is_empty() && modified.is_empty() {
        return Err(ToolError::new("empty_input"));
    }
    if original.len() > MAX_LINES || modified.len() > MAX_LINES {
        return Err(ToolError::new("too_many_lines"));
    }
    let a: Vec<String> = original.iter().map(|l| normalize(l, &params)).collect();
    let b: Vec<String> = modified.iter().map(|l| normalize(l, &params)).collect();
    let edits = script(&a, &b, cx)?;
    let (mut added, mut removed, mut unchanged) = (0_i64, 0_i64, 0_i64);
    let mut lines = Vec::with_capacity(edits.len());
    for edit in &edits {
        match edit {
            Edit::Same(i) => {
                unchanged += 1;
                lines.push(format!("  {}", original[*i]));
            }
            Edit::Removed(i) => {
                removed += 1;
                lines.push(format!("- {}", original[*i]));
            }
            Edit::Added(j) => {
                added += 1;
                lines.push(format!("+ {}", modified[*j]));
            }
        }
    }
    cx.progress(1.0);
    Ok(Outputs {
        diff: lines.join("\n"),
        added,
        removed,
        unchanged,
    })
}
