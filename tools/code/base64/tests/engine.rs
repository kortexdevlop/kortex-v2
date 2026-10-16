use kortex::runtime::{Cx, block_on};
use tool_code_base64::{Inputs, Params, engine::run};

fn go(text: &str, tweak: impl FnOnce(&mut Params)) -> Result<String, &'static str> {
    let mut p = Params::default();
    tweak(&mut p);
    block_on(run(
        Inputs {
            text: text.to_owned(),
        },
        p,
        &mut Cx::new(),
    ))
    .map(|o| o.text)
    .map_err(|e| e.code)
}

fn dec(text: &str) -> Result<String, &'static str> {
    go(text, |p| p.direction = "decode".into())
}

#[test]
fn rfc4648_test_vectors_encode() {
    for (plain, b64) in [
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(go(plain, |_| {}), Ok(b64.into()), "{plain}");
        assert_eq!(dec(b64), Ok(plain.into()), "{b64}");
    }
}

#[test]
fn unicode_roundtrip() {
    let s = "Ça va très bien ✓ 😀";
    let enc = go(s, |_| {}).expect("encode");
    assert_eq!(dec(&enc), Ok(s.into()));
}

#[test]
fn url_safe_alphabet_and_padding_option() {
    let s = "??>>~~";
    assert_eq!(go(s, |_| {}), Ok("Pz8+Pn5+".into()));
    assert_eq!(go(s, |p| p.url_safe = true), Ok("Pz8-Pn5-".into()));
    assert_eq!(go("f", |p| p.padding = false), Ok("Zg".into()));
    assert_eq!(
        dec("Pz8-Pn5-"),
        Ok(s.into()),
        "decoding accepts both alphabets"
    );
}

#[test]
fn decoding_tolerates_whitespace_and_missing_padding() {
    assert_eq!(dec("Zm9v\nYmFy"), Ok("foobar".into()));
    assert_eq!(dec("  Zm9vYg  "), Ok("foob".into()));
}

#[test]
fn invalid_input_is_reported() {
    assert_eq!(dec("not base64!"), Err("invalid_base64"));
    assert_eq!(
        dec("Zm9vY"),
        Err("invalid_base64"),
        "a lone trailing sextet is impossible"
    );
    assert_eq!(dec("Zg==Zg"), Err("invalid_base64"), "data after padding");
    assert_eq!(dec("/w=="), Err("not_text"), "0xFF is not UTF-8");
    assert_eq!(go("", |_| {}), Err("empty_input"));
    assert_eq!(
        go("x", |p| p.direction = "sideways".into()),
        Err("invalid_param")
    );
}
