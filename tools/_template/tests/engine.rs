use {{crate}}::engine::{ToolError, run};

#[test]
fn nominal() {
    assert_eq!(run("hello"), Ok("hello".to_owned()));
}

#[test]
fn error_on_empty_input() {
    assert_eq!(run(""), Err(ToolError::EmptyInput));
}
