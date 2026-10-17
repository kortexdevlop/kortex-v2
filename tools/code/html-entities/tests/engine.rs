use kortex::runtime::{Cx, block_on};
use tool_code_html_entities::{Inputs, Params, engine::run};
#[test]
fn nominal() {
    let o = block_on(run(
        Inputs {
            text: "<a & \"b\">".into(),
        },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("escape");
    assert_eq!(o.text, "&lt;a &amp; &quot;b&quot;&gt;");
    let o = block_on(run(
        Inputs {
            text: "&#x1F600; &copy; &amp;lt;".into(),
        },
        Params {
            direction: "decode".into(),
        },
        &mut Cx::new(),
    ))
    .expect("decode");
    assert_eq!(o.text, "😀 &copy; &lt;");
}
#[test]
fn invalid() {
    assert!(
        block_on(run(
            Inputs {
                text: "&#xD800;".into()
            },
            Params {
                direction: "decode".into()
            },
            &mut Cx::new()
        ))
        .is_err()
    );
}
