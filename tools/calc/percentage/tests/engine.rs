use kortex::runtime::{Cx, block_on};
use tool_calc_percentage::{Inputs, Params, engine::run};

#[test]
fn reference_operations() {
    for (op, a, b, want) in [
        ("part", 200.0, 15.0, 30.0),
        ("share", 30.0, 200.0, 15.0),
        ("change", 80.0, 100.0, 25.0),
        ("increase", 100.0, 20.0, 120.0),
        ("decrease", 100.0, 20.0, 80.0),
    ] {
        let o = block_on(run(
            Inputs {},
            Params {
                a,
                b,
                operation: op.into(),
            },
            &mut Cx::new(),
        ))
        .expect("calculation");
        assert!((o.result - want).abs() < 1e-9);
    }
}
#[test]
fn invalid_and_zero_denominators() {
    for p in [
        Params {
            a: 0.0,
            operation: "change".into(),
            ..Params::default()
        },
        Params {
            b: 0.0,
            operation: "share".into(),
            ..Params::default()
        },
        Params {
            a: f64::NAN,
            ..Params::default()
        },
    ] {
        assert!(block_on(run(Inputs {}, p, &mut Cx::new())).is_err());
    }
}
