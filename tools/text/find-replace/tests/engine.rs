use kortex::runtime::{Cx, block_on};
use tool_text_find_replace::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs {
            text: "abc abc".into(),
        },
        Params {
            search: "abc".into(),
            replacement: "X".into(),
            ..Params::default()
        },
        &mut Cx::new(),
    ))
    .expect("replace");
    assert_eq!(o.text, "X X");
    assert_eq!(o.count, 2);
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs { text: "abc".into() },
            Params::default(),
            &mut Cx::new()
        ))
        .is_err()
    );
}
