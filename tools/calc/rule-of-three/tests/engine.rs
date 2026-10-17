use kortex::runtime::{Cx, block_on};
use tool_calc_rule_of_three::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("calculate");
    assert!((o.result - 25.0).abs() < 1e-9);
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {},
            Params {
                a: 0.0,
                ..Params::default()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
