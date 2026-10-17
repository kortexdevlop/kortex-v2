use kortex::runtime::{Cx, block_on};
use tool_color_contrast_checker::{Inputs, Params, engine::run};

#[test]
fn black_white_and_identical() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("contrast");
    assert_eq!(o.ratio, "21.000000:1");
    assert_eq!(o.aaa_normal, "pass");
    let o = block_on(run(
        Inputs {},
        Params {
            foreground: "#fff".into(),
            background: "#fff".into(),
        },
        &mut Cx::new(),
    ))
    .expect("same");
    assert_eq!(o.ratio, "1.000000:1");
    assert_eq!(o.aa_large, "fail");
}
#[test]
fn boundary_not_rounded_and_errors() {
    let o = block_on(run(
        Inputs {},
        Params {
            foreground: "#777".into(),
            background: "#fff".into(),
        },
        &mut Cx::new(),
    ))
    .expect("contrast");
    assert_eq!(o.aa_normal, "fail");
    assert_eq!(o.aa_large, "pass");
    assert!(
        block_on(run(
            Inputs {},
            Params {
                foreground: "oops".into(),
                ..Params::default()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
