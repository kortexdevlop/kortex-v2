use kortex::runtime::{Cx, block_on};
use tool_code_json_formatter::{Inputs, Params, engine::run};

fn go(
    text: &str,
    tweak: impl FnOnce(&mut Params),
) -> Result<(String, String), (String, Option<String>)> {
    let mut p = Params::default();
    tweak(&mut p);
    block_on(run(
        Inputs {
            text: text.to_owned(),
        },
        p,
        &mut Cx::new(),
    ))
    .map(|o| (o.result, o.status))
    .map_err(|e| (e.code.to_owned(), e.detail))
}

#[test]
fn formats_with_the_requested_indent_and_keeps_key_order() {
    let r = go(r#"{"b":1,"a":[1,2],"c":{"x":null}}"#, |_| {}).expect("ok");
    assert_eq!(
        r.0,
        "{\n  \"b\": 1,\n  \"a\": [\n    1,\n    2\n  ],\n  \"c\": {\n    \"x\": null\n  }\n}"
    );
    assert_eq!(r.1, "valid");
    let four = go(r#"{"a":1}"#, |p| p.indent = 4).expect("ok");
    assert_eq!(four.0, "{\n    \"a\": 1\n}");
}

#[test]
fn minify_removes_all_insignificant_whitespace() {
    let r = go("{\n  \"a\" : [ 1 , 2 ],\n  \"b\" : \"x y\"\n}", |p| {
        p.action = "minify".into()
    })
    .expect("ok");
    assert_eq!(r.0, r#"{"a":[1,2],"b":"x y"}"#);
}

#[test]
fn validate_only_returns_no_text_but_a_status() {
    assert_eq!(
        go("[1,2,3]", |p| p.action = "validate".into()),
        Ok((String::new(), "valid".into()))
    );
}

#[test]
fn integers_and_decimals_keep_their_exact_spelling() {
    let r = go(r#"{"id":12345678901234567890,"f":1.10,"e":1E3}"#, |p| {
        p.action = "minify".into()
    })
    .expect("ok");
    assert_eq!(
        r.0, r#"{"id":12345678901234567890,"f":1.10,"e":1e+3}"#,
        "exponent notation is normalised, value unchanged"
    );
}

#[test]
fn sorting_keys_is_recursive_and_optional() {
    let r = go(r#"{"b":{"z":1,"y":2},"a":[{"d":1,"c":2}]}"#, |p| {
        p.action = "minify".into();
        p.sort_keys = true
    })
    .expect("ok");
    assert_eq!(r.0, r#"{"a":[{"c":2,"d":1}],"b":{"y":2,"z":1}}"#);
}

#[test]
fn unicode_and_escapes_survive() {
    let r = go(r#"{"s":"é\n\"ok\" 😀"}"#, |p| p.action = "minify".into()).expect("ok");
    assert_eq!(r.0, r#"{"s":"é\n\"ok\" 😀"}"#);
}

#[test]
fn errors_report_line_and_column() {
    let (code, detail) = go("{\n  \"a\": 1,\n  \"b\": }\n", |_| {}).expect_err("invalid");
    assert_eq!(code, "invalid_json");
    assert_eq!(detail.as_deref(), Some("3:8"));
    assert_eq!(go("   ", |_| {}).expect_err("empty").0, "empty_input");
    assert_eq!(
        go("{}", |p| p.indent = 99).expect_err("range").0,
        "invalid_param"
    );
    assert_eq!(
        go("{}", |p| p.action = "explode".into())
            .expect_err("action")
            .0,
        "invalid_param"
    );
    assert!(
        go("[1,]", |_| {}).is_err()
            && go("{'a':1}", |_| {}).is_err()
            && go("{\"a\":1} extra", |_| {}).is_err()
    );
}
