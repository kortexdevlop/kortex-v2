use kortex_seek::engine::{Doc, search};

#[test]
fn the_app_exposes_the_shared_search_engine() {
    assert!(search(&[Doc::default()], "", "fr", 5).is_empty());
}

#[test]
fn a_request_with_no_catalogue_finds_nothing() {
    assert!(search(&[], "mot de passe", "fr", 5).is_empty());
}
