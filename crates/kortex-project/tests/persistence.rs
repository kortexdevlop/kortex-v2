#![cfg(feature = "persistence")]
use kortex_contract::v2::{Contract, Operation};
use kortex_platform::projects::{
    MAX_SNAPSHOT_BYTES, SnapshotStore, StoreError, StoredSnapshot, validate_update,
};
use kortex_project::{
    Change, Command, Grants, Project, Source,
    persistence::{DurableSession, PersistenceError},
};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Waker},
};

fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    match future.as_mut().poll(&mut Context::from_waker(waker)) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("test backend unexpectedly yielded"),
    }
}

#[derive(Clone, Default)]
struct Fake {
    snapshot: Rc<RefCell<Option<StoredSnapshot>>>,
    failure: Rc<RefCell<Option<StoreError>>>,
    fail_after_write: Rc<Cell<bool>>,
    pending_after_write: Rc<Cell<bool>>,
    writes: Rc<Cell<usize>>,
}
impl SnapshotStore for Fake {
    async fn load(&self, _: &str) -> Result<Option<StoredSnapshot>, StoreError> {
        Ok(self.snapshot.borrow().clone())
    }
    async fn compare_and_swap(
        &self,
        _: &str,
        expected: Option<u64>,
        snapshot: &StoredSnapshot,
    ) -> Result<(), StoreError> {
        validate_update(expected, snapshot)?;
        if let Some(error) = self.failure.borrow().clone() {
            return Err(error);
        }
        if self.snapshot.borrow().as_ref().map(|s| s.revision) != expected {
            return Err(StoreError::Conflict);
        }
        self.snapshot.replace(Some(snapshot.clone()));
        self.writes.set(self.writes.get() + 1);
        if self.pending_after_write.get() {
            std::future::pending::<()>().await;
        }
        if self.fail_after_write.get() {
            return Err(StoreError::Unavailable);
        }
        Ok(())
    }
}

fn operation() -> Operation {
    let legacy =
        kortex_contract::parse(include_str!("../../../tools/text/reverse/tool.toml")).unwrap();
    let mut operation = Contract::from_v1(&legacy).unwrap().operations.remove(0);
    operation.id = "edit".into();
    operation.permissions.clipboard = "none".into();
    operation.effects.mutates_project = true;
    operation.effects.undoable = true;
    operation
}
fn command(revision: u64) -> Command {
    Command {
        id: format!("command-{revision}"),
        project_id: "demo".into(),
        expected_revision: revision,
        operation: "edit".into(),
        source: Source::Ui,
        changes: vec![Change::SetProperty {
            key: "title".into(),
            value: format!("Title {revision}").into(),
        }],
    }
}
fn create(store: Fake) -> DurableSession<Fake> {
    ready(DurableSession::create(
        store,
        Project::new("demo".into()),
        8,
        4096,
    ))
    .unwrap()
}

#[test]
fn apply_undo_redo_are_committed_before_reopen_and_history_is_volatile() {
    let store = Fake::default();
    let mut session = create(store.clone());
    let op = operation();
    let cmd = command(0);
    ready(session.apply(cmd.clone(), &op, &Grants::default())).unwrap();
    let count = store.writes.get();
    assert!(
        ready(session.apply(cmd, &op, &Grants::default()))
            .unwrap()
            .replayed
    );
    assert_eq!(store.writes.get(), count);
    ready(session.undo(1)).unwrap();
    assert!(session.project().properties.is_empty());
    assert_eq!(store.snapshot.borrow().as_ref().unwrap().revision, 2);
    ready(session.redo(2)).unwrap();
    let expected = session.project().clone();
    drop(session);
    let mut reopened = ready(DurableSession::load(store.clone(), "demo", 8, 4096)).unwrap();
    assert_eq!(reopened.project(), &expected);
    assert_eq!(
        ready(reopened.undo(3)),
        Err(PersistenceError::Project(
            kortex_project::Error::HistoryEmpty
        ))
    );
    assert!(
        ready(DurableSession::create(
            store,
            Project::new("demo".into()),
            8,
            4096
        ))
        .is_err()
    );
}

#[test]
fn stale_writers_keep_previous_state_and_require_reopen() {
    let store = Fake::default();
    let mut first = create(store.clone());
    let mut stale = ready(DurableSession::load(store.clone(), "demo", 8, 4096)).unwrap();
    ready(first.apply(command(0), &operation(), &Grants::default())).unwrap();
    assert_eq!(
        ready(stale.apply(command(0), &operation(), &Grants::default())),
        Err(PersistenceError::Store(StoreError::Conflict))
    );
    assert_eq!(stale.project().revision, 0);
    assert!(stale.needs_reload());
    assert_eq!(
        ready(stale.apply(command(0), &operation(), &Grants::default())),
        Err(PersistenceError::NeedsReload)
    );
    assert_eq!(store.snapshot.borrow().as_ref().unwrap().revision, 1);
}

#[test]
fn ambiguous_storage_error_preserves_memory_and_reopen_observes_commit() {
    let store = Fake::default();
    let mut session = create(store.clone());
    store.fail_after_write.set(true);
    assert_eq!(
        ready(session.apply(command(0), &operation(), &Grants::default())),
        Err(PersistenceError::Store(StoreError::Unavailable))
    );
    assert_eq!(session.project().revision, 0);
    assert!(session.needs_reload());
    let reopened = ready(DurableSession::load(store, "demo", 8, 4096)).unwrap();
    assert_eq!(reopened.project().revision, 1);
}

#[test]
fn dropping_pending_write_forces_reload_without_exposing_candidate() {
    let store = Fake::default();
    let mut session = create(store.clone());
    store.pending_after_write.set(true);
    let op = operation();
    let grants = Grants::default();
    let mut future = Box::pin(session.apply(command(0), &op, &grants));
    let waker = Waker::noop();
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(waker))
            .is_pending()
    );
    drop(future);
    assert!(session.needs_reload());
    assert_eq!(session.project().revision, 0);
    assert_eq!(store.snapshot.borrow().as_ref().unwrap().revision, 1);
}

#[test]
fn invalid_payloads_are_rejected_before_exposing_state() {
    let store = Fake::default();
    assert!(matches!(
        ready(DurableSession::load(store.clone(), "demo", 8, 4096)),
        Err(PersistenceError::NotFound)
    ));
    assert!(matches!(
        ready(DurableSession::load(store.clone(), "../demo", 8, 4096)),
        Err(PersistenceError::Store(StoreError::InvalidId))
    ));
    for (id, revision, schema) in [("other", 0, 1), ("demo", 1, 1), ("demo", 0, 99)] {
        let mut project = Project::new(id.into());
        project.revision = revision;
        project.schema = schema;
        store.snapshot.replace(Some(StoredSnapshot {
            revision: 0,
            bytes: serde_json::to_vec(&project).unwrap(),
        }));
        assert!(ready(DurableSession::load(store.clone(), "demo", 8, 4096)).is_err());
    }
    store.snapshot.replace(Some(StoredSnapshot {
        revision: 0,
        bytes: vec![b'x'; MAX_SNAPSHOT_BYTES + 1],
    }));
    assert!(matches!(
        ready(DurableSession::load(store, "demo", 8, 4096)),
        Err(PersistenceError::Store(StoreError::TooLarge))
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_reopen_and_cross_instance_cas_preserve_winner() {
    use kortex_platform::projects::NativeStore;
    let root = std::env::temp_dir().join(format!("kortex-project-persist-{}", std::process::id()));
    let store = NativeStore::open(&root).unwrap();
    let mut first = ready(DurableSession::create(
        store.clone(),
        Project::new("demo".into()),
        8,
        4096,
    ))
    .unwrap();
    let mut stale = ready(DurableSession::load(
        NativeStore::open(&root).unwrap(),
        "demo",
        8,
        4096,
    ))
    .unwrap();
    ready(first.apply(command(0), &operation(), &Grants::default())).unwrap();
    assert_eq!(
        ready(stale.apply(command(0), &operation(), &Grants::default())),
        Err(PersistenceError::Store(StoreError::Conflict))
    );
    let expected = first.project().clone();
    drop(first);
    drop(stale);
    let reopened = ready(DurableSession::load(store, "demo", 8, 4096)).unwrap();
    assert_eq!(reopened.project(), &expected);
    // Test-owned temporary directory only.
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_parallel_create_has_one_winner_and_bounded_corrupt_reads() {
    use kortex_platform::projects::NativeStore;
    let root = std::env::temp_dir().join(format!(
        "kortex-project-cas-parallel-{}",
        std::process::id()
    ));
    let store = NativeStore::open(&root).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let threads: Vec<_> = (0..4)
        .map(|i| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                ready(store.compare_and_swap(
                    "race",
                    None,
                    &StoredSnapshot {
                        revision: 0,
                        bytes: vec![i],
                    },
                ))
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(StoreError::Conflict))
            .count(),
        3
    );
    assert_eq!(ready(store.load("race")).unwrap().unwrap().revision, 0);
    assert_eq!(ready(store.load("../race")), Err(StoreError::InvalidId));
    std::fs::write(root.join("broken.snapshot"), b"invalid").unwrap();
    assert_eq!(ready(store.load("broken")), Err(StoreError::Corrupt));
    std::fs::write(
        root.join("huge.snapshot"),
        vec![0_u8; MAX_SNAPSHOT_BYTES + 17],
    )
    .unwrap();
    assert_eq!(ready(store.load("huge")), Err(StoreError::TooLarge));
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
