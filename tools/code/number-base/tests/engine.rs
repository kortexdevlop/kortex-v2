use kortex::runtime::{Cx, block_on};
use tool_code_number_base::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs { text: "255".into() },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("base");
    assert_eq!(o.text, "ff");
    let o = block_on(run(
        Inputs {
            text: i128::MIN.to_string(),
        },
        Params {
            to_base: 10,
            ..Params::default()
        },
        &mut Cx::new(),
    ))
    .expect("min");
    assert_eq!(o.text, i128::MIN.to_string());
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs { text: "2".into() },
            Params {
                from_base: 2,
                ..Params::default()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
