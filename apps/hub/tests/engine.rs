use kortex_hub::engine::{Dashboard, definitions};
#[test]
fn favorites_recents_and_launches_survive_roundtrip_without_unknown_tools() {
    let mut s = Dashboard::default();
    s.favorite("code.json-formatter");
    s.record("code.json-formatter");
    s.record("secure.hash-calculator");
    s.record("code.json-formatter");
    assert_eq!(s.total(), 3);
    assert_eq!(s.recent.len(), 2);
    let mut restored: Dashboard =
        serde_json::from_str(&serde_json::to_string(&s).expect("persist")).expect("load");
    restored.reconcile(&["code.json-formatter".into()]);
    assert_eq!(restored.favorites.len(), 1);
    assert_eq!(restored.recent, vec!["code.json-formatter"]);
    assert_eq!(restored.total(), 2);
    restored.favorite("code.json-formatter");
    assert!(restored.favorites.is_empty());
}
#[test]
fn widget_contracts_and_keyboard_ordering_are_consistent() {
    let contracts = definitions();
    assert_eq!(contracts.len(), 4);
    for c in &contracts {
        assert!(c.name.contains_key("fr") && c.name.contains_key("en"));
    }
    let mut s = Dashboard::default();
    s.move_by("favorites", 1);
    assert_eq!(s.widgets[1].id, "favorites");
    s.reorder("catalogue", "recent");
    assert_eq!(s.widgets[0].id, "catalogue");
    s.widgets[0].wide = true;
    let restored: Dashboard =
        serde_json::from_str(&serde_json::to_string(&s).expect("persist")).expect("restore");
    assert!(restored.widgets[0].wide);
}
