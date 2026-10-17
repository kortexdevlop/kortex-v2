use kortex::runtime::{Cx, block_on};
use tool_text_cleaner::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs {
            text: "  a\t b\r\nc\u{200b} ".into(),
        },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("clean");
    assert_eq!(o.text, "a b\nc");
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {
                text: String::new()
            },
            Params::default(),
            &mut Cx::new()
        ))
        .is_err()
    );
}
