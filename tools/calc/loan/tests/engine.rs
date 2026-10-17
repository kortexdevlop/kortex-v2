use kortex::runtime::{Cx, block_on};
use tool_calc_loan::{Inputs, Params, engine::run};

#[test]
fn zero_rate_and_insurance() {
    let o = block_on(run(
        Inputs {},
        Params {
            principal: 1200.0,
            years: 1,
            annual_rate: 0.0,
            insurance_rate: 1.0,
        },
        &mut Cx::new(),
    ))
    .expect("loan");
    assert!((o.monthly - 101.0).abs() < 1e-9);
    assert_eq!(o.schedule.rows.len(), 12);
    assert_eq!(o.schedule.rows[11][3], "0.00");
    assert!((o.total - 1212.0).abs() < 1e-8);
}
#[test]
fn standard_fixed_rate_reference() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("loan");
    assert!((o.monthly - 554.5975978539207).abs() < 1e-8);
    assert!((o.interest - 33103.423484940955).abs() < 1e-6);
    assert_eq!(o.schedule.rows[239][3], "0.00");
}
#[test]
fn tiny_rate_and_invalid_values() {
    let o = block_on(run(
        Inputs {},
        Params {
            annual_rate: 1e-10,
            ..Params::default()
        },
        &mut Cx::new(),
    ))
    .expect("stable");
    assert!((o.monthly - 100000.0 / 240.0).abs() < 0.001);
    for p in [
        Params {
            principal: -1.0,
            ..Params::default()
        },
        Params {
            annual_rate: f64::NAN,
            ..Params::default()
        },
    ] {
        assert!(block_on(run(Inputs {}, p, &mut Cx::new())).is_err());
    }
}
