use kortex::runtime::{Cx, block_on};
use tool_calc_compound_interest::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("calculate");
    assert!((o.final_value - 1647.00949769028).abs() < 1e-7);
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {},
            Params {
                annual_rate: f64::NAN,
                ..Params::default()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
