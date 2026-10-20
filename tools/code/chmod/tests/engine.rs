use kortex::runtime::{Cx, block_on};
use tool_code_chmod::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs { text: "755".into() },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("result");
    assert_eq!(o.text, "rwxr-xr-x");
}
#[test]
fn invalid() {
    for text in ["888", "4755", "rwx"] {
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
