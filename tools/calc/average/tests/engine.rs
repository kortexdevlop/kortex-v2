use kortex::runtime::{Cx, block_on};
use tool_calc_average::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs {
            text: "1; 2; 3; 4".into(),
        },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("result");
    assert_eq!(o.count, 4);
    assert_eq!(o.mean, 2.5);
    assert_eq!(o.median, 2.5);
}
#[test]
fn invalid() {
    for text in ["", "NaN", "1;garbage"] {
        assert!(
            block_on(run(
                Inputs { text: text.into() },
                Params::default(),
                &mut Cx::new()
            ))
            .is_err()
        );
    }
}
