//! Local snapshot persistence. Legacy `save`/`load` require a single writer;
//! `DurableSession` uses compare-and-swap across host instances.
//! History, retry receipts and resource capabilities are never persisted.
use crate::{Project, Session};
use kortex_platform::projects::{
    MAX_SNAPSHOT_BYTES, SnapshotStore, StoreError, StoredSnapshot, validate_id,
};
use kortex_platform::{Storage, storage::MAX_VALUE_BYTES};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistenceError {
    Store(StoreError),
    Project(crate::Error),
    NotFound,
    Corrupt,
    NeedsReload,
}
impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PersistenceError {}

/// Durable metadata only. The host owns media access and invokes a local SnapshotStore backend.
/// The public project remains the last confirmed snapshot after storage failures.
/// Reopen after any uncertain write, conflict or cancelled write future; never retry blind.
pub struct DurableSession<S: SnapshotStore> {
    store: S,
    session: Session,
    needs_reload: bool,
}

impl<S: SnapshotStore> DurableSession<S> {
    pub async fn create(
        store: S,
        project: Project,
        max_history: usize,
        max_metadata_bytes: usize,
    ) -> Result<Self, PersistenceError> {
        validate_id(&project.id).map_err(PersistenceError::Store)?;
        if project.revision != 0 {
            return Err(PersistenceError::Project(crate::Error::InvalidCommand));
        }
        let session = Session::open(
            project,
            max_history,
            max_metadata_bytes.min(MAX_SNAPSHOT_BYTES),
        )
        .map_err(PersistenceError::Project)?;
        let snapshot = encode(session.project())?;
        store
            .compare_and_swap(&session.project().id, None, &snapshot)
            .await
            .map_err(PersistenceError::Store)?;
        Ok(Self {
            store,
            session,
            needs_reload: false,
        })
    }

    pub async fn load(
        store: S,
        id: &str,
        max_history: usize,
        max_metadata_bytes: usize,
    ) -> Result<Self, PersistenceError> {
        validate_id(id).map_err(PersistenceError::Store)?;
        let snapshot = store
            .load(id)
            .await
            .map_err(PersistenceError::Store)?
            .ok_or(PersistenceError::NotFound)?;
        if snapshot.bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(PersistenceError::Store(StoreError::TooLarge));
        }
        let project: Project =
            serde_json::from_slice(&snapshot.bytes).map_err(|_| PersistenceError::Corrupt)?;
        if project.id != id || project.revision != snapshot.revision {
            return Err(PersistenceError::Corrupt);
        }
        let session = Session::open(
            project,
            max_history,
            max_metadata_bytes.min(MAX_SNAPSHOT_BYTES),
        )
        .map_err(PersistenceError::Project)?;
        Ok(Self {
            store,
            session,
            needs_reload: false,
        })
    }

    pub fn project(&self) -> &Project {
        self.session.project()
    }
    pub fn needs_reload(&self) -> bool {
        self.needs_reload
    }

    pub async fn apply(
        &mut self,
        command: crate::Command,
        operation: &kortex_contract::v2::Operation,
        grants: &crate::Grants,
    ) -> Result<crate::Receipt, PersistenceError> {
        self.check_ready()?;
        let mut candidate = self.session.clone();
        let receipt = candidate
            .apply(command, operation, grants)
            .map_err(PersistenceError::Project)?;
        // An in-memory replay does not mutate the snapshot and does not write it again.
        if !receipt.replayed {
            self.persist(candidate).await?;
        }
        Ok(receipt)
    }

    pub async fn undo(&mut self, expected_revision: u64) -> Result<(), PersistenceError> {
        self.check_ready()?;
        let mut candidate = self.session.clone();
        candidate
            .undo(expected_revision)
            .map_err(PersistenceError::Project)?;
        self.persist(candidate).await
    }

    pub async fn redo(&mut self, expected_revision: u64) -> Result<(), PersistenceError> {
        self.check_ready()?;
        let mut candidate = self.session.clone();
        candidate
            .redo(expected_revision)
            .map_err(PersistenceError::Project)?;
        self.persist(candidate).await
    }

    fn check_ready(&self) -> Result<(), PersistenceError> {
        if self.needs_reload {
            Err(PersistenceError::NeedsReload)
        } else {
            Ok(())
        }
    }

    async fn persist(&mut self, candidate: Session) -> Result<(), PersistenceError> {
        let snapshot = encode(candidate.project())?;
        // Set before await: dropping a pending write future also forces a reopen.
        self.needs_reload = true;
        self.store
            .compare_and_swap(
                &candidate.project().id,
                Some(self.session.project().revision),
                &snapshot,
            )
            .await
            .map_err(PersistenceError::Store)?;
        self.session = candidate;
        self.needs_reload = false;
        Ok(())
    }
}

fn encode(project: &Project) -> Result<StoredSnapshot, PersistenceError> {
    let bytes = serde_json::to_vec(project).map_err(|_| PersistenceError::Corrupt)?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(PersistenceError::Store(StoreError::TooLarge));
    }
    Ok(StoredSnapshot {
        revision: project.revision,
        bytes,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Platform(String),
    InvalidSnapshot,
    TooLarge,
    Project(crate::Error),
}

/// Store metadata only. The host must serialize writers; resource IDs grant no file access.
pub fn save(store: &mut impl Storage, session: &Session) -> Result<(), Error> {
    let project = session.project();
    let bytes = serde_json::to_string(project).map_err(|_| Error::InvalidSnapshot)?;
    if bytes.len() > MAX_VALUE_BYTES {
        return Err(Error::TooLarge);
    }
    store
        .set(&key(&project.id)?, &bytes)
        .map_err(|e| Error::Platform(e.0))
}

/// Reopen a validated snapshot with empty undo/redo and retry history.
pub fn load(store: &impl Storage, id: &str, max_history: usize) -> Result<Option<Session>, Error> {
    let Some(bytes) = store.get(&key(id)?).map_err(|e| Error::Platform(e.0))? else {
        return Ok(None);
    };
    if bytes.len() > MAX_VALUE_BYTES {
        return Err(Error::TooLarge);
    }
    let project: Project = serde_json::from_str(&bytes).map_err(|_| Error::InvalidSnapshot)?;
    if project.id != id {
        return Err(Error::InvalidSnapshot);
    }
    Session::open(project, max_history, MAX_VALUE_BYTES)
        .map(Some)
        .map_err(Error::Project)
}

fn key(id: &str) -> Result<String, Error> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(Error::InvalidSnapshot);
    }
    Ok(format!("project-{id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kortex_platform::MemoryPlatform;
    #[test]
    fn reopen_and_reject_corrupt_identity_schema_and_oversize() {
        let mut store = MemoryPlatform::default();
        let session = Session::open(Project::new("demo".into()), 8, MAX_VALUE_BYTES).unwrap();
        save(&mut store, &session).unwrap();
        assert_eq!(
            load(&store, "demo", 8).unwrap().unwrap().project(),
            session.project()
        );
        assert!(load(&store, "missing", 8).unwrap().is_none());
        store.set("project-demo", "{}").unwrap();
        assert!(matches!(
            load(&store, "demo", 8),
            Err(Error::InvalidSnapshot)
        ));
        let mut project = Project::new("other".into());
        store
            .set("project-demo", &serde_json::to_string(&project).unwrap())
            .unwrap();
        assert!(load(&store, "demo", 8).is_err());
        project.id = "demo".into();
        project.schema = 99;
        store
            .set("project-demo", &serde_json::to_string(&project).unwrap())
            .unwrap();
        assert!(matches!(
            load(&store, "demo", 8),
            Err(Error::Project(crate::Error::UnsupportedSchema))
        ));
        store
            .set("project-demo", &"x".repeat(MAX_VALUE_BYTES + 1))
            .unwrap();
        assert!(matches!(load(&store, "demo", 8), Err(Error::TooLarge)));
        assert!(load(&store, "../bad", 8).is_err());
    }
}
