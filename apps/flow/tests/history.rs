//! The run history keeps the shape of a run, never its data.
use std::collections::{BTreeMap, BTreeSet};

use kortex_flow::history::{KEEP, Outcome, Runs, duration_label, record, utc_label};

fn names(items: &[&str]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|i| ((*i).to_owned(), format!("Card {i}")))
        .collect()
}
fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn a_successful_run_lists_every_card_with_its_state_and_time() {
    let order = names(&["n1", "n2", "n3"]);
    let ms = BTreeMap::from([("n2".to_owned(), 840_u64)]);
    let r = record(&Outcome {
        workflow: "Flux",
        at: 1_000,
        total_ms: 900,
        order: &order,
        ms: &ms,
        done: &set(&["n1", "n2"]),
        skipped: &set(&["n3"]),
        failed: None,
    });
    assert!(r.ok);
    assert_eq!(
        r.nodes
            .iter()
            .map(|n| (n.node.as_str(), n.state.as_str(), n.ms))
            .collect::<Vec<_>>(),
        [("n1", "done", 0), ("n2", "done", 840), ("n3", "skipped", 0)]
    );
    assert_eq!(r.workflow, "Flux");
}

#[test]
fn a_failed_run_names_the_card_and_the_cards_after_it_did_not_run() {
    let order = names(&["n1", "n2", "n3"]);
    let r = record(&Outcome {
        workflow: "Flux",
        at: 2_000,
        total_ms: 50,
        order: &order,
        ms: &BTreeMap::new(),
        done: &set(&["n1"]),
        skipped: &set(&[]),
        failed: Some(("n2", format!("http_status (404) {}", "x".repeat(300)))),
    });
    assert!(!r.ok);
    assert_eq!(
        r.nodes.iter().map(|n| n.state.as_str()).collect::<Vec<_>>(),
        ["done", "failed", "not_run"]
    );
    assert!(
        r.nodes[1]
            .error
            .as_deref()
            .is_some_and(|e| e.starts_with("http_status (404)") && e.chars().count() == 120),
        "error codes are kept, long details are cut"
    );
    // The record holds no field that could carry what a card produced.
    let json = serde_json::to_string(&r).expect("json");
    assert!(!json.contains("value") && !json.contains("output"));
}

#[test]
fn the_history_is_bounded_newest_first_and_can_be_cleaned() {
    let mut runs = Runs::default();
    let order = names(&["n1"]);
    for at in 1..=(KEEP as u64 + 5) {
        runs.push(record(&Outcome {
            workflow: "F",
            at,
            total_ms: 1,
            order: &order,
            ms: &BTreeMap::new(),
            done: &set(&["n1"]),
            skipped: &set(&[]),
            failed: None,
        }));
    }
    assert_eq!(runs.items.len(), KEEP);
    assert_eq!(runs.items[0].id, KEEP as u64 + 5, "newest first");
    runs.remove(runs.items[0].id);
    assert_eq!(runs.items.len(), KEEP - 1);
    runs.clear();
    assert!(runs.items.is_empty());
}

#[test]
fn dates_and_durations_read_naturally() {
    assert_eq!(utc_label(0), "1970-01-01 00:00 UTC");
    assert_eq!(utc_label(1_792_439_707_989), "2026-10-19 19:55 UTC");
    assert_eq!(
        utc_label(951_782_400_000 + 3_600_000 * 13 + 60_000 * 5),
        "2000-02-29 13:05 UTC",
        "a leap day"
    );
    assert_eq!(duration_label(850), "850 ms");
    assert_eq!(duration_label(2_449), "2.4 s");
}
