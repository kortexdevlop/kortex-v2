use kortex::runtime::{Cx, block_on};
use tool_text_slug_generator::{Inputs, Params, engine::run};

fn slug(text: &str, p: Params) -> Result<String, &'static str> {
    block_on(run(Inputs { text: text.into() }, p, &mut Cx::new()))
        .map(|o| o.text)
        .map_err(|e| e.code)
}
#[test]
fn accents_and_separators() {
    assert_eq!(
        slug("  L’été — déjà là ! Œuf & Straße", Params::default()),
        Ok("l-ete-deja-la-oeuf-strasse".into())
    );
    assert_eq!(slug("e\u{301}té", Params::default()), Ok("ete".into()));
}
#[test]
fn unicode_and_truncation() {
    assert_eq!(
        slug(
            "你好 世界",
            Params {
                ascii_only: false,
                max_length: 4,
                ..Params::default()
            }
        ),
        Ok("你好-世".into())
    );
    assert_eq!(
        slug(
            "Hello World",
            Params {
                separator: "underscore".into(),
                max_length: 6,
                ..Params::default()
            }
        ),
        Ok("hello".into())
    );
}
#[test]
fn errors() {
    assert!(slug("!!!", Params::default()).is_err());
    assert!(slug("", Params::default()).is_err());
}
