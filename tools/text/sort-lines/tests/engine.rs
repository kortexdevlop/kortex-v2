use kortex::runtime::{Cx, block_on};
use tool_text_sort_lines::{Inputs, Params, engine::run};

fn go(text: &str, tweak: impl FnOnce(&mut Params)) -> Result<(String, i64, i64), &'static str> {
    let mut p = Params::default();
    tweak(&mut p);
    block_on(run(
        Inputs {
            text: text.to_owned(),
        },
        p,
        &mut Cx::new(),
    ))
    .map(|o| (o.text, o.lines, o.removed))
    .map_err(|e| e.code)
}

#[test]
fn defaults_sort_ascending_and_dedupe() {
    assert_eq!(
        go("pear\napple\npear\nbanana", |_| {}),
        Ok(("apple\nbanana\npear".into(), 3, 1))
    );
}

#[test]
fn dedupe_keeps_the_first_occurrence_in_original_order() {
    assert_eq!(
        go("b\na\nb\nc\na", |p| p.sort = "none".into()),
        Ok(("b\na\nc".into(), 3, 2))
    );
}

#[test]
fn ignore_case_applies_to_dedupe_and_sort() {
    let r = go("Banana\napple\nbanana\nApple", |p| p.ignore_case = true);
    assert_eq!(r, Ok(("apple\nBanana".into(), 2, 2)));
    assert_eq!(
        go("a\nA", |p| p.sort = "none".into()),
        Ok(("a\nA".into(), 2, 0)),
        "case matters by default"
    );
}

#[test]
fn trim_and_empty_line_options() {
    assert_eq!(
        go("  x  \n\n y\nx", |p| p.sort = "asc".into()),
        Ok(("x\ny".into(), 2, 2))
    );
    assert_eq!(
        go("a\n\nb", |p| {
            p.remove_empty = false;
            p.sort = "none".into();
            p.dedupe = false
        }),
        Ok(("a\n\nb".into(), 3, 0))
    );
    assert_eq!(
        go(" a\na", |p| {
            p.trim = false;
            p.dedupe = true;
            p.sort = "none".into()
        }),
        Ok((" a\na".into(), 2, 0))
    );
}

#[test]
fn other_sorts() {
    assert_eq!(
        go("a\nc\nb", |p| p.sort = "desc".into()),
        Ok(("c\nb\na".into(), 3, 0))
    );
    assert_eq!(
        go("ccc\na\nbb", |p| p.sort = "length".into()),
        Ok(("a\nbb\nccc".into(), 3, 0))
    );
    assert_eq!(
        go("1\n2\n3", |p| p.sort = "reverse".into()),
        Ok(("3\n2\n1".into(), 3, 0))
    );
}

#[test]
fn windows_line_endings_and_errors() {
    assert_eq!(go("b\r\na\r\nb", |_| {}), Ok(("a\nb".into(), 2, 1)));
    assert_eq!(go("   \n  ", |_| {}), Err("empty_input"));
    assert_eq!(
        go("a", |p| p.sort = "sideways".into()),
        Err("invalid_param")
    );
}
