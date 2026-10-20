use kortex::runtime::{Cx, block_on};
use tool_calc_vat::{Inputs, Params, engine::run};

#[test]
fn net_gross_and_zero_rate() {
    for (amount, rate, direction, net, gross) in [
        (100.0, 20.0, "net", 100.0, 120.0),
        (120.0, 20.0, "gross", 100.0, 120.0),
        (100.0, 0.0, "net", 100.0, 100.0),
        (105.5, 5.5, "gross", 100.0, 105.5),
    ] {
        let o = block_on(run(
            Inputs {},
            Params {
                amount,
                rate,
                direction: direction.into(),
            },
            &mut Cx::new(),
        ))
        .expect("tax");
        assert!((o.net - net).abs() < 1e-8);
        assert!((o.gross - gross).abs() < 1e-8);
        assert!((o.tax - (gross - net)).abs() < 1e-8);
    }
}
#[test]
fn rejects_invalid() {
    for p in [
        Params {
            amount: -1.0,
            ..Params::default()
        },
        Params {
            rate: f64::INFINITY,
            ..Params::default()
        },
    ] {
        assert!(block_on(run(Inputs {}, p, &mut Cx::new())).is_err());
    }
}
