use kortex::runtime::{Cx, block_on};
use tool_text_word_counter::{Inputs, Params, engine::run};

use tool_text_word_counter::Outputs;

fn count(text: &str) -> Outputs {
    block_on(run(
        Inputs {
            text: text.to_owned(),
        },
        Params::default(),
        &mut Cx::new(),
    ))
    .expect("counts")
}

#[test]
fn nominal_counts() {
    let o = count("Hello world. Bye!");
    assert_eq!(
        (o.words, o.sentences, o.characters, o.characters_no_spaces),
        (3, 2, 17, 15)
    );
    assert_eq!((o.paragraphs, o.lines), (1, 1));
}

#[test]
fn empty_text_is_all_zero() {
    let o = count("");
    assert_eq!(
        (o.words, o.characters, o.sentences, o.paragraphs, o.lines),
        (0, 0, 0, 0, 0)
    );
    assert!(o.reading_minutes.abs() < f64::EPSILON);
}

#[test]
fn unicode_characters_are_counted_as_characters_not_bytes() {
    let o = count("été ça 😀");
    assert_eq!((o.words, o.characters), (3, 8));
}

#[test]
fn paragraphs_and_lines() {
    let o = count("one\ntwo\n\nthree\n\n\n\nfour");
    assert_eq!((o.paragraphs, o.lines, o.words), (3, 8, 4));
}

#[test]
fn decimals_ellipsis_and_missing_final_punctuation_are_not_extra_sentences() {
    assert_eq!(count("Pi is 3.14 today").sentences, 1);
    assert_eq!(count("Wait... what? Yes").sentences, 3);
    assert_eq!(count("...").sentences, 0);
    assert_eq!(count("Done.").sentences, 1);
}

#[test]
fn reading_time_follows_the_speed_and_is_validated() {
    let text = "word ".repeat(460);
    let o = block_on(run(Inputs { text }, Params { wpm: 230 }, &mut Cx::new())).expect("counts");
    assert!((o.reading_minutes - 2.0).abs() < 0.001);
    let bad = block_on(run(
        Inputs { text: "x".into() },
        Params { wpm: 10 },
        &mut Cx::new(),
    ))
    .map_err(|e| e.code);
    assert_eq!(bad.map(|_| ()), Err("invalid_param"));
}
