use kortex::runtime::{Cx, block_on};
use tool_convert_units::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs {},
        Params {
            from_unit: "mi".into(),
            to_unit: "km".into(),
            ..Params::default()
        },
        &mut Cx::new(),
    ))
    .expect("distance");
    assert!((o.result - 1.609344).abs() < 1e-12);
    let o = block_on(run(
        Inputs {},
        Params {
            value: 32.0,
            from_unit: "f".into(),
            to_unit: "c".into(),
        },
        &mut Cx::new(),
    ))
    .expect("temperature");
    assert!(o.result.abs() < 1e-9);
}
#[test]
fn invalid() {
    for p in [
        Params {
            to_unit: "kg".into(),
            ..Params::default()
        },
        Params {
            value: -1.0,
            from_unit: "k".into(),
            to_unit: "c".into(),
        },
    ] {
        assert!(block_on(run(Inputs {}, p, &mut Cx::new())).is_err());
    }
}
