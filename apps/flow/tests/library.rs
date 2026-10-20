//! Saved workflows and the shipped templates.
use kortex_flow::catalogue::ports;
use kortex_flow::engine::{NodeKind, Workflow, validate};
use kortex_flow::library::{Library, new_id};
use kortex_flow::templates::all;

fn named(name: &str) -> Workflow {
    Workflow {
        name: name.into(),
        ..Workflow::default()
    }
}
fn id(n: &str) -> impl FnOnce() -> String {
    let n = n.to_owned();
    move || n
}

#[test]
fn saving_replaces_in_place_and_new_ids_are_distinct() {
    let mut lib = Library::default();
    let a = lib.save(None, "  Mon flux ", &named("x"), 10, id("w1"));
    assert_eq!(a, "w1");
    assert_eq!(lib.get("w1").map(|s| s.name.as_str()), Some("Mon flux"));
    let again = lib.save(Some("w1"), "Mon flux v2", &named("y"), 20, id("w2"));
    assert_eq!(again, "w1", "the same id is kept");
    assert_eq!(lib.items.len(), 1);
    assert_eq!(
        lib.get("w1").map(|s| (s.name.as_str(), s.updated_at)),
        Some(("Mon flux v2", 20))
    );
    assert_ne!(new_id(), new_id());
    assert!(new_id().starts_with('w') && new_id().len() == 13);
}

#[test]
fn duplicate_rename_and_delete_keep_a_tombstone() {
    let mut lib = Library::default();
    lib.save(None, "Flux", &named("Flux"), 10, id("w1"));
    let copy = lib.duplicate("w1", "(copie)", 20, id("w2")).expect("copy");
    assert_eq!(
        lib.get(&copy).map(|s| s.name.as_str()),
        Some("Flux (copie)")
    );
    lib.rename("w2", "Autre", 30);
    assert_eq!(
        lib.get("w2").map(|s| s.workflow.name.as_str()),
        Some("Autre")
    );
    lib.remove("w1", 40);
    assert!(lib.get("w1").is_none());
    assert_eq!(lib.items.len(), 2, "the tombstone stays");
    assert_eq!(lib.visible().len(), 1);
    assert!(
        lib.duplicate("w1", "(copie)", 50, id("w3")).is_none(),
        "a deleted workflow cannot be copied"
    );
}

#[test]
fn merging_two_devices_keeps_the_newest_change_and_deletions() {
    let mut here = Library::default();
    here.save(None, "A", &named("A"), 100, id("a"));
    here.save(None, "B", &named("B"), 100, id("b"));
    let mut there = here.clone();
    there.rename("a", "A depuis le téléphone", 200);
    there.remove("b", 300);
    there.save(None, "C", &named("C"), 250, id("c"));
    here.rename("a", "A ancien", 150);
    here.merge(&there, 400);
    assert_eq!(
        here.get("a").map(|s| s.name.as_str()),
        Some("A depuis le téléphone")
    );
    assert!(
        here.get("b").is_none(),
        "the deletion wins over the older copy"
    );
    assert_eq!(here.get("c").map(|s| s.name.as_str()), Some("C"));
    // Merging is idempotent and an older copy never overwrites a newer one.
    let snapshot = here.clone();
    here.merge(&there, 400);
    assert_eq!(here, snapshot);
    // Old tombstones are forgotten.
    here.merge(&Library::default(), 400 + 31 * 24 * 3600 * 1000);
    assert!(here.items.iter().all(|s| !s.deleted));
}

#[test]
fn every_template_is_valid_in_both_languages_and_says_what_it_needs() {
    let templates = all();
    assert_eq!(templates.len(), 12, "all the shipped files parse");
    let mut seen = std::collections::BTreeSet::new();
    for t in templates {
        assert!(seen.insert(t.id.clone()), "unique id {}", t.id);
        for lang in ["fr", "en"] {
            assert!(
                !t.name(lang).is_empty() && !t.description(lang).is_empty(),
                "{} {lang}",
                t.id
            );
            let workflow = t.instantiate(lang);
            let issues = validate(&workflow, ports());
            assert!(
                issues.is_empty(),
                "{} {lang}: {:?}",
                t.id,
                issues
                    .iter()
                    .map(|i| (i.code, &i.detail))
                    .collect::<Vec<_>>()
            );
            assert!(workflow.nodes.iter().all(|n| n.pos.is_some()), "laid out");
            assert_eq!(workflow.name, t.name(lang));
            for node in &workflow.nodes {
                if let NodeKind::Text { value } = &node.kind {
                    assert!(
                        !value.contains("{sample}") && !value.is_empty(),
                        "{} has a sample text",
                        t.id
                    );
                }
            }
        }
        let wants_file = t
            .workflow
            .nodes
            .iter()
            .any(|n| matches!(n.kind, NodeKind::File { .. }));
        assert_eq!(
            t.needs.iter().any(|n| n == "files"),
            wants_file,
            "{}: needs files",
            t.id
        );
        let wants_ai = t
            .workflow
            .nodes
            .iter()
            .any(|n| matches!(&n.kind, NodeKind::Logic { op, .. } if op == "ai"));
        assert_eq!(
            t.needs.iter().any(|n| n == "ai"),
            wants_ai,
            "{}: needs ai",
            t.id
        );
    }
}
