use kortex_contract::v2::{Contract, Operation};
use kortex_project::{
    Change, Command, Error, Grants, Project, Resource, ResourceId, Session, Source,
};
use serde_json::json;

fn operation() -> Operation {
    let legacy =
        kortex_contract::parse(include_str!("../../../tools/text/reverse/tool.toml")).unwrap();
    let mut op = Contract::from_v1(&legacy).unwrap().operations.remove(0);
    op.id = "edit".into();
    op.permissions.clipboard = "none".into();
    op.effects.mutates_project = true;
    op.effects.undoable = true;
    op
}
fn command(source: Source, revision: u64, id: &str, changes: Vec<Change>) -> Command {
    Command {
        id: id.into(),
        project_id: "local-project".into(),
        expected_revision: revision,
        operation: "edit".into(),
        source,
        changes,
    }
}
fn session() -> Session {
    Session::open(Project::new("local-project".into()), 8, 4096).unwrap()
}

#[test]
fn ui_flow_and_mind_share_commands_and_never_copy_media_bytes() {
    let id = ResourceId::try_from("host_token_001".to_owned()).unwrap();
    let changes = vec![
        Change::AttachResource {
            resource: Resource {
                id: id.clone(),
                mime: "video/mp4".into(),
                size_bytes: 40_000_000_000,
            },
        },
        Change::SetProperty {
            key: "title".into(),
            value: json!("Shared project"),
        },
    ];
    let grants = Grants {
        resources: vec![id],
        ..Default::default()
    };
    let op = operation();
    let mut snapshots = vec![];
    for source in [Source::Ui, Source::Flow, Source::Mind] {
        let mut session = session();
        let cmd = command(source, 0, "change-1", changes.clone());
        // Same serialized command boundary for all hosts; only provenance differs.
        let wire = serde_json::to_string(&cmd).unwrap();
        assert!(wire.len() < 1024);
        let decoded = serde_json::from_str(&wire).unwrap();
        assert_eq!(session.apply(decoded, &op, &grants).unwrap().revision, 1);
        assert_eq!(session.last_source(), Some(source));
        let snapshot = serde_json::to_string(session.project()).unwrap();
        let restored: Project = serde_json::from_str(&snapshot).unwrap();
        assert_eq!(
            Session::open(restored, 8, 4096).unwrap().project(),
            session.project()
        );
        snapshots.push(snapshot);
        session.undo(1).unwrap();
        assert!(session.project().resources.is_empty());
        session.redo(2).unwrap();
        assert_eq!(
            session.project().properties.get("title"),
            Some(&json!("Shared project"))
        );
    }
    assert!(snapshots.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn rejects_stale_revisions_and_commits_all_changes_or_none() {
    let mut session = session();
    let op = operation();
    let grants = Grants::default();
    let set = Change::SetProperty {
        key: "title".into(),
        value: json!("first"),
    };
    let cmd = command(Source::Flow, 0, "one", vec![set.clone()]);
    session.apply(cmd.clone(), &op, &grants).unwrap();
    assert!(session.apply(cmd, &op, &grants).unwrap().replayed);
    assert_eq!(
        session.apply(
            command(Source::Ui, 0, "two", vec![set.clone()]),
            &op,
            &grants
        ),
        Err(Error::RevisionConflict)
    );
    let before = session.project().clone();
    let invalid = command(
        Source::Mind,
        1,
        "three",
        vec![
            set,
            Change::SetProperty {
                key: "".into(),
                value: json!(123),
            },
        ],
    );
    assert_eq!(
        session.apply(invalid, &op, &grants),
        Err(Error::InvalidProperty)
    );
    assert_eq!(session.project(), &before);
}

#[test]
fn requires_explicit_grants_and_rejects_paths_and_oversized_metadata() {
    assert!(ResourceId::try_from("C:\\private\\clip.mp4".to_owned()).is_err());
    assert!(ResourceId::try_from("https://example.com/file".to_owned()).is_err());
    let mut session = session();
    let mut op = operation();
    op.permissions.sensitive = true;
    let cmd = command(
        Source::Mind,
        0,
        "one",
        vec![Change::SetProperty {
            key: "a".into(),
            value: json!("x".repeat(5000)),
        }],
    );
    assert_eq!(
        session.apply(cmd.clone(), &op, &Grants::default()),
        Err(Error::PermissionDenied)
    );
    assert_eq!(
        session.apply(
            cmd,
            &op,
            &Grants {
                sensitive: true,
                ..Default::default()
            }
        ),
        Err(Error::MetadataLimit)
    );
    assert_eq!(session.project().revision, 0);
}

#[test]
fn job_cancellation_requires_backend_acknowledgement() {
    use kortex_project::jobs::{Error, Job, State};
    let mut job = Job::new(true);
    job.start().unwrap();
    job.report(5000).unwrap();
    assert_eq!(job.report(4000), Err(Error::InvalidProgress));
    job.request_cancel().unwrap();
    assert_eq!(job.state(), State::CancelRequested);
    assert_eq!(job.finish(true), Err(Error::InvalidTransition));
    job.acknowledge_cancel().unwrap();
    assert_eq!(job.state(), State::Cancelled);
    assert!(job.start().is_err());
    let mut non_cancellable = Job::new(false);
    non_cancellable.start().unwrap();
    assert_eq!(non_cancellable.request_cancel(), Err(Error::NotCancellable));
}

#[test]
fn reads_and_unknown_permission_values_cannot_bypass_host_grants() {
    let mut session = session();
    let mut op = operation();
    let cmd = command(
        Source::Mind,
        0,
        "read",
        vec![Change::SetProperty {
            key: "a".into(),
            value: json!(1),
        }],
    );
    op.permissions.filesystem = "read".into();
    assert_eq!(
        session.apply(cmd.clone(), &op, &Grants::default()),
        Err(Error::PermissionDenied)
    );
    op.permissions.filesystem = "none".into();
    op.permissions.clipboard = "read".into();
    assert_eq!(
        session.apply(cmd.clone(), &op, &Grants::default()),
        Err(Error::PermissionDenied)
    );
    op.permissions.clipboard = "unknown".into();
    assert_eq!(
        session.apply(cmd, &op, &Grants::default()),
        Err(Error::PermissionDenied)
    );
    assert_eq!(session.project().revision, 0);
}
