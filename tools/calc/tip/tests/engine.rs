use kortex::runtime::{Cx, block_on};
use tool_calc_tip::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("calculate");
    assert!((o.per_person - 57.5).abs() < 1e-9);
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
