use kortex::runtime::{Cx, block_on};
use tool_text_repeat::{Inputs, Params, engine::run};

fn repeat(text: &str, count: i64, separator: &str) -> Result<String, &'static str> {
    block_on(run(
        Inputs {
            text: text.to_owned(),
        },
        Params {
            count,
            separator: separator.to_owned(),
        },
        &mut Cx::new(),
    ))
    .map(|o| o.text)
    .map_err(|e| e.code)
}

#[test]
fn repeats_with_a_separator() {
    assert_eq!(repeat("ha", 3, "-"), Ok("ha-ha-ha".to_owned()));
    assert_eq!(repeat("été", 2, ""), Ok("étéété".to_owned()));
    assert_eq!(repeat("a", 1, ", "), Ok("a".to_owned()));
}

#[test]
fn errors_are_typed() {
    assert_eq!(repeat("", 3, " "), Err("empty_input"));
    assert_eq!(repeat("a", 0, " "), Err("invalid_param"));
    assert_eq!(repeat(&"x".repeat(10_000), 1000, " "), Err("too_large"));
}
