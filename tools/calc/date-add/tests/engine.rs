use kortex::runtime::{Cx, block_on};
use tool_calc_date_add::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("result");
    assert_eq!(o.date, "2024-02-29");
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {},
            Params {
                date: "9999-12-31".into(),
                ..Params::default()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
