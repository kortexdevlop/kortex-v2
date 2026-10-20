use kortex::runtime::{Cx, block_on};
use tool_color_converter::{Inputs, Params, engine::run};

#[test]
fn red_reference_and_oklch() {
    let o = block_on(run(
        Inputs {
            text: "#f00".into(),
        },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("color");
    assert_eq!(o.hex, "#FF0000");
    assert_eq!(o.rgb, "rgb(255 0 0)");
    assert_eq!(o.gamut, "inside");
    let back = block_on(run(
        Inputs { text: o.oklch },
        Params {
            format: "oklch".into(),
        },
        &mut Cx::new(),
    ))
    .expect("back");
    assert_eq!(back.hex, "#FF0000");
}
#[test]
fn rejects_invalid_and_reports_clipping() {
    assert!(
        block_on(run(
            Inputs { text: "red".into() },
            Params::default(),
            &mut Cx::new()
        ))
        .is_err()
    );
    let o = block_on(run(
        Inputs {
            text: "0.7 0.4 30".into(),
        },
        Params {
            format: "oklch".into(),
        },
        &mut Cx::new(),
    ))
    .expect("color");
    assert_eq!(o.gamut, "clipped");
}
