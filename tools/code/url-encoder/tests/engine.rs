use kortex::runtime::{Cx, block_on};
use tool_code_url_encoder::{Inputs, Params, engine::run};

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

fn dec(t: &str) -> Result<String, &'static str> {
    go(t, |p| p.direction = "decode".into())
}

#[test]
fn component_encoding_matches_encodeuricomponent() {
    assert_eq!(
        go("a b&c=d/é?", |_| {}),
        Ok("a%20b%26c%3Dd%2F%C3%A9%3F".into())
    );
    assert_eq!(
        go("safe-_.!~*'()AZaz09", |_| {}),
        Ok("safe-_.!~*'()AZaz09".into())
    );
}

#[test]
fn uri_scope_keeps_separators() {
    let u = "https://x.fr/a b?q=é&r=1#top";
    assert_eq!(
        go(u, |p| p.scope = "uri".into()),
        Ok("https://x.fr/a%20b?q=%C3%A9&r=1#top".into())
    );
}

#[test]
fn decoding_roundtrips_and_handles_utf8() {
    let s = "Ça va? 100% ✓ 😀 a+b";
    let enc = go(s, |_| {}).expect("encode");
    assert_eq!(dec(&enc), Ok(s.into()));
    assert_eq!(dec("a%20b%C3%A9"), Ok("a bé".into()));
}

#[test]
fn plus_is_literal_unless_form_mode_is_on() {
    assert_eq!(dec("a+b"), Ok("a+b".into()));
    assert_eq!(
        go("a+b%2Bc", |p| {
            p.direction = "decode".into();
            p.plus_as_space = true
        }),
        Ok("a b+c".into())
    );
}

#[test]
fn bad_sequences_and_invalid_utf8_are_reported() {
    assert_eq!(dec("100%"), Err("invalid_percent"));
    assert_eq!(dec("%zz"), Err("invalid_percent"));
    assert_eq!(dec("%4"), Err("invalid_percent"));
    assert_eq!(dec("%FF"), Err("not_text"));
    assert_eq!(go("", |_| {}), Err("empty_input"));
    assert_eq!(go("x", |p| p.direction = "x".into()), Err("invalid_param"));
}
