use kortex::runtime::{Cx, block_on};
use tool_text_diff::{Inputs, Params, engine::run};

fn diff(
    a: &str,
    b: &str,
    ignore_case: bool,
    ignore_whitespace: bool,
) -> Result<tool_text_diff::Outputs, &'static str> {
    block_on(run(
        Inputs {
            original: a.into(),
            modified: b.into(),
        },
        Params {
            ignore_case,
            ignore_whitespace,
        },
        &mut Cx::new(),
    ))
    .map_err(|e| e.code)
}

#[test]
fn nominal_marks_added_removed_and_unchanged_lines() {
    let out = diff(
        "un\ndeux\ntrois",
        "un\ndeux modifié\ntrois\nquatre",
        false,
        false,
    )
    .expect("diff");
    assert_eq!(out.diff, "  un\n- deux\n+ deux modifié\n  trois\n+ quatre");
    assert_eq!((out.added, out.removed, out.unchanged), (2, 1, 2));
}

#[test]
fn identical_texts_have_no_differences() {
    let out = diff("a\nb\nc", "a\nb\nc", false, false).expect("diff");
    assert_eq!((out.added, out.removed, out.unchanged), (0, 0, 3));
    assert!(!out.diff.contains("+ ") && !out.diff.contains("- "));
}

#[test]
fn case_and_whitespace_can_be_ignored() {
    assert_eq!(
        diff("Bonjour\nle monde", "bonjour\nle monde", false, false)
            .expect("diff")
            .removed,
        1
    );
    assert_eq!(
        diff("Bonjour\nle monde", "bonjour\nle monde", true, false)
            .expect("diff")
            .removed,
        0
    );
    assert_eq!(
        diff("a   b\n\tc", "a b\nc", false, false)
            .expect("diff")
            .removed,
        2
    );
    assert_eq!(
        diff("a   b\n\tc", "a b\nc", false, true)
            .expect("diff")
            .removed,
        0
    );
}

#[test]
fn a_deleted_block_and_an_empty_side() {
    let out = diff("a\nb\nc\nd", "a\nd", false, false).expect("diff");
    assert_eq!(out.diff, "  a\n- b\n- c\n  d");
    let all_added = diff("", "x\ny", false, false).expect("diff");
    assert_eq!((all_added.added, all_added.removed), (2, 0));
}

#[test]
fn errors_are_typed() {
    assert_eq!(diff("", "", false, false).map(|_| ()), Err("empty_input"));
    let big = "x\n".repeat(4001);
    assert_eq!(
        diff(&big, "a", false, false).map(|_| ()),
        Err("too_many_lines")
    );
}
