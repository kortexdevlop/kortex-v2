//! `cargo run -p kortex-project --example shared_project`
//! Exercises metadata only, without claiming to decode or render a video.
use kortex_contract::v2::Contract;
use kortex_project::{Change, Command, Grants, Project, Resource, ResourceId, Session, Source};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let legacy = kortex_contract::parse(include_str!("../../../tools/text/reverse/tool.toml"))?;
    let mut contract = Contract::from_v1(&legacy).map_err(|issues| format!("{issues:?}"))?;
    let operation = &mut contract.operations[0];
    operation.id = "edit".into();
    operation.permissions.clipboard = "none".into();
    operation.effects.mutates_project = true;
    operation.effects.undoable = true;
    let id = ResourceId::try_from("local_host_media_1".to_owned())?;
    let grants = Grants {
        resources: vec![id.clone()],
        ..Default::default()
    };
    let mut session = Session::open(Project::new("example".into()), 16, 8192)?;
    for (index, source) in [Source::Ui, Source::Flow, Source::Mind]
        .into_iter()
        .enumerate()
    {
        let changes = if index == 0 {
            vec![Change::AttachResource {
                resource: Resource {
                    id: id.clone(),
                    mime: "video/mp4".into(),
                    size_bytes: 40_000_000_000,
                },
            }]
        } else {
            vec![Change::SetProperty {
                key: format!("label_{index}"),
                value: format!("Edited by {source:?}").into(),
            }]
        };
        let command = Command {
            id: format!("command_{index}"),
            project_id: "example".into(),
            expected_revision: session.project().revision,
            operation: "edit".into(),
            source,
            changes,
        };
        session.apply(command, operation, &grants)?;
        println!(
            "{source:?}: project revision {}",
            session.project().revision
        );
    }
    session.undo(3)?;
    session.redo(4)?;
    let snapshot = serde_json::to_string_pretty(session.project())?;
    println!(
        "Local metadata snapshot ({} bytes), media remain external:\n{snapshot}",
        snapshot.len()
    );
    Ok(())
}
