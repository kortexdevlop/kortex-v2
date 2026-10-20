use kortex::runtime::{Cx, block_on};
use tool_text_lorem_ipsum::{Inputs, Params, engine::run};

#[test]
fn exact_counts_and_classic_start() {
    let o = block_on(run(Inputs {}, Params::default(), &mut Cx::new())).expect("lorem");
    assert!(o.text.starts_with("Lorem ipsum dolor sit amet"));
    assert_eq!(o.words, 180);
    assert_eq!(o.text.split("\n\n").count(), 3);
    assert_eq!(o.text.split_whitespace().count(), 180);
}
#[test]
fn words_and_limits() {
    let o = block_on(run(
        Inputs {},
        Params {
            unit: "words".into(),
            count: 17,
            ..Params::default()
        },
        &mut Cx::new(),
    ))
    .expect("words");
    assert_eq!(o.text.split_whitespace().count(), 17);
    for p in [
        Params {
            count: 101,
            ..Params::default()
        },
        Params {
            count: 0,
            ..Params::default()
        },
    ] {
        assert!(block_on(run(Inputs {}, p, &mut Cx::new())).is_err());
    }
}
