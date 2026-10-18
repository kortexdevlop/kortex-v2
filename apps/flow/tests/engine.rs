//! The application's own path to the shared workflow engine, and the Flow-specific catalogue
//! (the engine's rules are tested in `kortex-workflow`).
use kortex_flow::catalogue::{ports, usable_tools};
use kortex_flow::engine::{FORMAT_VERSION, Workflow, from_json, to_json, validate};

#[test]
fn the_app_exposes_the_shared_engine_and_round_trips_a_workflow() {
    let workflow = Workflow::default();
    assert_eq!(workflow.version, FORMAT_VERSION);
    assert_eq!(from_json(&to_json(&workflow)), Ok(workflow.clone()));
    assert!(
        validate(&workflow, ports()).is_empty(),
        "an empty workflow is valid"
    );
}

#[test]
fn the_catalogue_offers_every_text_only_tool_and_none_that_needs_a_file() {
    let offered: Vec<String> = usable_tools("fr").into_iter().map(|(id, _)| id).collect();
    assert!(offered.contains(&"text.cleaner".to_owned()));
    assert!(
        !offered.contains(&"pdf.merge".to_owned()),
        "pdf.merge needs files"
    );
    assert!(ports().contains_key("text.cleaner"));
}
