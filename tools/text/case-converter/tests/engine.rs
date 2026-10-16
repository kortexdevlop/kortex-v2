use kortex::runtime::{Cx, block_on};
use tool_text_case_converter::{Inputs, Params, engine::run};

fn conv(text: &str, case: &str) -> Result<String, &'static str> {
    block_on(run(
        Inputs {
            text: text.to_owned(),
        },
        Params {
            case: case.to_owned(),
        },
        &mut Cx::new(),
    ))
    .map(|o| o.text)
    .map_err(|e| e.code)
}

#[test]
fn simple_cases() {
    assert_eq!(conv("Hello World", "upper"), Ok("HELLO WORLD".into()));
    assert_eq!(conv("Hello World", "lower"), Ok("hello world".into()));
}

#[test]
fn title_case_handles_mixed_input_and_apostrophes() {
    assert_eq!(
        conv("the quick BROWN fox", "title"),
        Ok("The Quick Brown Fox".into())
    );
    assert_eq!(
        conv("don't stop, l'été", "title"),
        Ok("Don't Stop, L'été".into())
    );
}

#[test]
fn sentence_case_restarts_after_terminators() {
    assert_eq!(
        conv("hELLO wORLD. nEW one! really? yes", "sentence"),
        Ok("Hello world. New one! Really? Yes".into())
    );
    assert_eq!(
        conv("3.14 is pi. okay", "sentence"),
        Ok("3.14 is pi. Okay".into())
    );
}

#[test]
fn identifier_cases_split_on_separators_and_case_changes() {
    assert_eq!(conv("hello world", "snake"), Ok("hello_world".into()));
    assert_eq!(
        conv("helloWorld foo-bar", "kebab"),
        Ok("hello-world-foo-bar".into())
    );
    assert_eq!(conv("Hello big_world", "camel"), Ok("helloBigWorld".into()));
    assert_eq!(
        conv("hello big_world", "pascal"),
        Ok("HelloBigWorld".into())
    );
    assert_eq!(conv("max retries", "constant"), Ok("MAX_RETRIES".into()));
}

#[test]
fn accents_and_special_letters_convert_correctly() {
    assert_eq!(
        conv("élève à l'école", "upper"),
        Ok("ÉLÈVE À L'ÉCOLE".into())
    );
    assert_eq!(
        conv("Ça Va Très Bien", "snake"),
        Ok("ça_va_très_bien".into())
    );
}

#[test]
fn errors() {
    assert_eq!(conv("", "upper"), Err("empty_input"));
    assert_eq!(conv("x", "sideways"), Err("invalid_param"));
}
