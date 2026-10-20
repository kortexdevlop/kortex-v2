use {{crate}}::engine::normalize;

#[test]
fn normalizes_spacing_and_case() {
    assert_eq!(normalize("  Hello   WORLD "), "hello world");
}

#[test]
fn empty_query_stays_empty() {
    assert_eq!(normalize("   "), "");
}
