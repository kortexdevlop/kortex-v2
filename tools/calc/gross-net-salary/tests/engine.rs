use kortex::runtime::{Cx, block_on};
use tool_calc_gross_net_salary::{Inputs, Outputs, Params, engine::run};

fn calc(p: Params) -> Result<Outputs, &'static str> {
    block_on(run(Inputs {}, p, &mut Cx::new())).map_err(|e| e.code)
}

fn gross(amount: f64) -> Params {
    Params {
        amount,
        ..Params::default()
    }
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn known_case_2500_gross_non_cadre() {
    // Hand-computed from the 2026 rates documented in engine.rs.
    // cotisations 282.75 ; CSG+CRDS 238.26 ; net 1978.99
    let o = calc(gross(2500.0)).expect("calc");
    assert!(close(o.social_contributions, 282.75, 0.01), "{o:?}");
    assert!(close(o.csg_crds, 238.26, 0.02), "{o:?}");
    assert!(close(o.net_before_tax, 1978.99, 0.03), "{o:?}");
    assert!(close(o.charges_rate, 20.84, 0.05), "{o:?}");
    assert_eq!(o.gross, 2500.0);
}

#[test]
fn above_pass_uses_tranche_2_ceg_and_cet() {
    let above = calc(gross(6000.0)).expect("calc");
    // 6000: T1 on 4005, T2 on 1995.
    let expected_social = 0.069 * 4005.0
        + 0.004 * 6000.0
        + 0.0315 * 4005.0
        + 0.0864 * 1995.0
        + 0.0086 * 4005.0
        + 0.0108 * 1995.0
        + 0.0014 * 6000.0;
    assert!(
        close(above.social_contributions, expected_social, 0.06),
        "{above:?} vs {expected_social}"
    );
}

#[test]
fn cadre_pays_slightly_more_apec() {
    let non = calc(gross(3000.0)).expect("calc");
    let cadre = calc(Params {
        cadre: true,
        ..gross(3000.0)
    })
    .expect("calc");
    assert!(close(
        cadre.social_contributions - non.social_contributions,
        0.72,
        0.02
    ));
}

#[test]
fn net_to_gross_inverts_gross_to_net() {
    for g in [1800.0, 2500.0, 4005.0, 7500.0, 15000.0] {
        let net = calc(gross(g)).expect("calc").net_before_tax;
        let back = calc(Params {
            amount: net,
            mode: "net_to_gross".into(),
            ..Params::default()
        })
        .expect("calc");
        assert!(
            close(back.gross, g, 0.05),
            "gross {g} -> net {net} -> gross {}",
            back.gross
        );
    }
}

#[test]
fn annual_is_twelve_times_monthly_and_tax_is_applied() {
    let m = calc(gross(2500.0)).expect("calc");
    let a = calc(Params {
        amount: 30000.0,
        period: "annual".into(),
        ..Params::default()
    })
    .expect("calc");
    assert!(
        close(a.net_before_tax, m.net_before_tax * 12.0, 0.2),
        "{a:?}"
    );
    let taxed = calc(Params {
        tax_rate: 10.0,
        ..gross(2500.0)
    })
    .expect("calc");
    assert!(close(taxed.income_tax, taxed.net_taxable * 0.10, 0.01));
    assert!(close(
        taxed.net_after_tax,
        taxed.net_before_tax - taxed.income_tax,
        0.011
    ));
}

#[test]
fn zero_amount_and_invalid_params() {
    let z = calc(gross(0.0)).expect("calc");
    assert_eq!((z.net_before_tax, z.charges_rate), (0.0, 0.0));
    assert_eq!(calc(gross(-1.0)).map(|_| ()), Err("invalid_param"));
    assert_eq!(
        calc(Params {
            tax_rate: 90.0,
            ..Params::default()
        })
        .map(|_| ()),
        Err("invalid_param")
    );
    assert_eq!(
        calc(Params {
            mode: "sideways".into(),
            ..Params::default()
        })
        .map(|_| ()),
        Err("invalid_param")
    );
}

#[test]
fn cet_step_at_the_pass_is_handled_by_the_inverse() {
    // Just above the PASS the net steps down (CET starts): a net inside the step has no exact
    // gross, and the smallest solution must be returned, never a higher one.
    let step_net = calc(gross(4005.0)).expect("calc").net_before_tax - 1.0;
    let back = calc(Params {
        amount: step_net,
        mode: "net_to_gross".into(),
        ..Params::default()
    })
    .expect("calc");
    assert!(back.gross <= 4005.0 + 0.01, "{back:?}");
}
