use kortex::runtime::{Cx, block_on};
use tool_calc_unit_price::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("calculate");
    assert!((o.unit_a - 6.0).abs() < 1e-9);
    assert!((o.unit_b - 5.0).abs() < 1e-9);
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {},
            Params {
                quantity_a: 0.0,
                ..Params::default()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
