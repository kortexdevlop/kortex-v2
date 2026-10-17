use kortex::runtime::{Cx, block_on};
use tool_calc_fuel_cost::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("result");
    assert_eq!(o.litres, 6.0);
    assert!((o.total - 10.8).abs() < 1e-9);
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {},
            Params {
                people: 0,
                ..Params::default()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
