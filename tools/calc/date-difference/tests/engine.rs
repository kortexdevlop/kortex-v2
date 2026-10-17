use kortex::runtime::{Cx, block_on};
use tool_calc_date_difference::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("result");
    assert_eq!(o.days, 31);
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {},
            Params {
                start: "2024-02-30".into(),
                ..Params::default()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
