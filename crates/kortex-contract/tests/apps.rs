use kortex_contract::{parse_app, validate_app};
const STORE: &str = include_str!("../../../apps/store/app.toml");
#[test]
fn app_uses_shared_metadata_ports_permissions_and_requires_both_targets() {
    let c = parse_app(STORE).expect("complete app contract");
    assert!(validate_app(&c).is_empty(), "{:?}", validate_app(&c));
    let mut missing = c.clone();
    missing.app.platforms = vec!["desktop".into()];
    assert!(
        validate_app(&missing)
            .iter()
            .any(|i| i.0.contains("web et desktop"))
    );
    let mut invalid = c;
    invalid.app.id = "wrong.store".into();
    assert!(!validate_app(&invalid).is_empty());
}
#[test]
fn reduced_or_misspelled_application_contracts_are_rejected() {
    assert!(parse_app("[app]\nid='apps.store'\nversion='0.1.0'").is_err());
    assert!(parse_app(&format!("{STORE}\n[unknown]\nvalue=true\n")).is_err());
    assert!(
        parse_app(&STORE.replace("ui = \"custom\"", "ui = \"custom\"\nmisspelled = true")).is_err()
    );
    let c = parse_app(&STORE.replace("clipboard = \"none\"", "clipboard = \"invalid\""))
        .expect("parse");
    assert!(!validate_app(&c).is_empty());
}
