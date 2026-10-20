use kortex::runtime::{Cx, block_on};
use tool_convert_timestamp::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs { text: "-1".into() },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("result");
    assert_eq!(o.text, "1969-12-31T23:59:59.000Z");
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {
                text: i64::MAX.to_string()
            },
            Params::default(),
            &mut Cx::new()
        ))
        .is_err()
    );
}
