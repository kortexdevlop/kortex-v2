use kortex::runtime::{Cx, block_on};
use tool_text_reverse::{Inputs, Params, engine::run};

fn reverse(text: &str, mode: &str) -> Result<String, &'static str> {
    block_on(run(
        Inputs {
            text: text.to_owned(),
        },
        Params {
            mode: mode.to_owned(),
        },
        &mut Cx::new(),
    ))
    .map(|o| o.text)
    .map_err(|e| e.code)
}

#[test]
fn reverses_the_letters() {
    assert_eq!(reverse("Bonjour", "characters"), Ok("ruojnoB".to_owned()));
    assert_eq!(reverse("été ça", "characters"), Ok("aç été".to_owned()));
}

#[test]
fn reverses_the_word_order_line_by_line() {
    assert_eq!(
        reverse("un deux  trois\nquatre cinq", "words"),
        Ok("trois deux un\ncinq quatre".to_owned())
    );
}

#[test]
fn reverses_the_line_order_and_keeps_a_final_newline() {
    assert_eq!(reverse("a\nb\nc", "lines"), Ok("c\nb\na".to_owned()));
    assert_eq!(reverse("a\nb\n", "lines"), Ok("b\na\n".to_owned()));
}

#[test]
fn errors_are_typed() {
    assert_eq!(reverse("", "characters"), Err("empty_input"));
    assert_eq!(reverse("abc", "sideways"), Err("invalid_param"));
}
