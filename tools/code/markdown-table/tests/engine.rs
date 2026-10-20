use kortex::runtime::{Cx, block_on};
use tool_code_markdown_table::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs {
            text: "Name	Age
Ada	36"
                .into(),
        },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("result");
    assert_eq!(
        o.text,
        "| Name | Age |
| :--- | :--- |
| Ada | 36 |"
    );
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {
                text: "a	b
c"
                .into()
            },
            Params::default(),
            &mut Cx::new()
        ))
        .is_err()
    );
}
