use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};

const WORDS: &str = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua ut enim ad minim veniam quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur excepteur sint occaecat cupidatat non proident sunt in culpa qui officia deserunt mollit anim id est laborum";
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let (paragraphs, per) = match params.unit.as_str() {
        "words" => (1, params.count),
        "paragraphs" if params.count <= 100 => (params.count, params.words_per_paragraph),
        _ => return Err(ToolError::new("invalid_param")),
    };
    let words: Vec<_> = WORDS.split_whitespace().collect();
    let mut blocks = Vec::new();
    for p in 0..paragraphs {
        cx.check_cancelled()?;
        let offset = if p == 0 && params.classic_start {
            0
        } else {
            (p as usize + 1) * 17
        };
        let mut text = (0..per)
            .map(|i| words[(i as usize + offset) % words.len()])
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(first) = text.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        text.push('.');
        blocks.push(text);
    }
    Ok(Outputs {
        text: blocks.join("\n\n"),
        words: paragraphs * per,
    })
}
