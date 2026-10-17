use kortex::runtime::{Cx, block_on};
use tool_calc_age::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("result");
    assert_eq!(o.years, 24);
    assert_eq!(o.days, 8766);
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {},
            Params {
                at: "1999-01-01".into(),
                ..Params::default()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
