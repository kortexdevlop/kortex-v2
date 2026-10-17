use kortex::runtime::{Cx, block_on};
use tool_text_deduplicate::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs {
            text: "A\nB\nA\na".into(),
        },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("unique");
    assert_eq!(o.text, "A\nB\na");
    assert_eq!(o.removed, 1);
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
