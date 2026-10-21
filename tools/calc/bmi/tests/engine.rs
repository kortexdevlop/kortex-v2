use kortex::runtime::{Cx, block_on};
use tool_calc_bmi::{Inputs, Params, engine::run};

fn bmi(weight_kg: f64, height_cm: f64) -> Result<tool_calc_bmi::Outputs, &'static str> {
    block_on(run(
        Inputs {},
        Params {
            weight_kg,
            height_cm,
        },
        &mut Cx::new(),
    ))
    .map_err(|e| e.code)
}

#[test]
fn nominal_computes_the_index_the_category_and_the_normal_range() {
    let out = bmi(70.0, 170.0).expect("bmi");
    assert_eq!(out.bmi, 24.2);
    assert_eq!(out.category, "normal");
    assert_eq!(out.healthy_min_kg, 53.5);
    assert_eq!(out.healthy_max_kg, 72.0);
}

#[test]
fn who_category_boundaries() {
    let category = |w: f64| bmi(w, 100.0).expect("bmi").category;
    // At 100 cm the index equals the weight.
    assert_eq!(category(15.9), "severe_thinness");
    assert_eq!(category(16.5), "moderate_thinness");
    assert_eq!(category(18.0), "mild_thinness");
    assert_eq!(category(18.5), "normal");
    assert_eq!(category(24.9), "normal");
    assert_eq!(category(25.0), "overweight");
    assert_eq!(category(30.0), "obesity_1");
    assert_eq!(category(35.0), "obesity_2");
    assert_eq!(category(40.0), "obesity_3");
}

#[test]
fn errors_are_typed() {
    assert_eq!(bmi(f64::NAN, 170.0).map(|_| ()), Err("invalid_param"));
    assert_eq!(bmi(70.0, 0.0).map(|_| ()), Err("invalid_param"));
    // Out of the contract's range.
    assert_eq!(bmi(0.0, 170.0).map(|_| ()), Err("invalid_param"));
}
