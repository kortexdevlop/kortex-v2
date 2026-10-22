//! Bounded local metadata snapshots. Backends must serialize compare-and-swap across writers.
pub const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSnapshot {
    pub revision: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    InvalidId,
    TooLarge,
    Conflict,
    Corrupt,
    Unavailable,
}
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StoreError {}

#[allow(async_fn_in_trait)]
pub trait SnapshotStore {
    async fn load(&self, id: &str) -> Result<Option<StoredSnapshot>, StoreError>;
    /// Create requires revision zero; updates require exactly the next revision.
    async fn compare_and_swap(
        &self,
        id: &str,
        expected: Option<u64>,
        snapshot: &StoredSnapshot,
    ) -> Result<(), StoreError>;
}

pub fn validate_id(id: &str) -> Result<(), StoreError> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(StoreError::InvalidId);
    }
    Ok(())
}

pub fn validate_update(expected: Option<u64>, snapshot: &StoredSnapshot) -> Result<(), StoreError> {
    if snapshot.bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(StoreError::TooLarge);
    }
    if Some(snapshot.revision) != expected.map_or(Some(0), |r| r.checked_add(1)) {
        return Err(StoreError::Conflict);
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub use crate::projects_web::WebStore;

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeStore;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    use std::{
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        path::{Path, PathBuf},
    };
    const MAGIC: &[u8; 8] = b"KXPRJ001";

    /// Root must be a host-controlled local directory, not a shared/untrusted download folder.
    /// Native operations are blocking and belong on the host's background thread.
    /// Existing directory permissions remain the host's responsibility.
    #[derive(Debug, Clone)]
    pub struct NativeStore {
        root: PathBuf,
    }

    fn unavailable(_: impl std::fmt::Debug) -> StoreError {
        StoreError::Unavailable
    }
    fn no_symlink(path: &Path) -> Result<(), StoreError> {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => Err(StoreError::Unavailable),
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(unavailable(e)),
        }
    }

    impl NativeStore {
        pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);
            builder.create(&root).map_err(unavailable)?;
            let root = root.as_ref().canonicalize().map_err(unavailable)?;
            if !root.is_dir() {
                return Err(StoreError::Unavailable);
            }
            Ok(Self { root })
        }
        fn lock(&self, id: &str) -> Result<File, StoreError> {
            validate_id(id)?;
            let path = self.root.join(format!("{id}.lock"));
            no_symlink(&path)?;
            let mut options = OpenOptions::new();
            options.read(true).write(true).create(true).truncate(false);
            #[cfg(unix)]
            options.mode(0o600);
            let lock = options.open(path).map_err(unavailable)?;
            lock.lock().map_err(unavailable)?;
            Ok(lock)
        }
        fn read_locked(&self, id: &str) -> Result<Option<StoredSnapshot>, StoreError> {
            let path = self.root.join(format!("{id}.snapshot"));
            no_symlink(&path)?;
            let file = match File::open(path) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(unavailable(e)),
            };
            let mut envelope = Vec::new();
            file.take((MAX_SNAPSHOT_BYTES + 17) as u64)
                .read_to_end(&mut envelope)
                .map_err(unavailable)?;
            if envelope.len() > MAX_SNAPSHOT_BYTES + 16 {
                return Err(StoreError::TooLarge);
            }
            if envelope.len() < 16 || &envelope[..8] != MAGIC {
                return Err(StoreError::Corrupt);
            }
            let revision = u64::from_le_bytes(
                envelope[8..16]
                    .try_into()
                    .map_err(|_| StoreError::Corrupt)?,
            );
            Ok(Some(StoredSnapshot {
                revision,
                bytes: envelope[16..].to_vec(),
            }))
        }
    }

    impl SnapshotStore for NativeStore {
        async fn load(&self, id: &str) -> Result<Option<StoredSnapshot>, StoreError> {
            let _lock = self.lock(id)?;
            self.read_locked(id)
        }
        async fn compare_and_swap(
            &self,
            id: &str,
            expected: Option<u64>,
            snapshot: &StoredSnapshot,
        ) -> Result<(), StoreError> {
            validate_id(id)?;
            validate_update(expected, snapshot)?;
            let _lock = self.lock(id)?;
            if self.read_locked(id)?.as_ref().map(|s| s.revision) != expected {
                return Err(StoreError::Conflict);
            }
            let mut random = [0_u8; 16];
            getrandom::fill(&mut random).map_err(unavailable)?;
            let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
            let temporary = self.root.join(format!("{id}-{suffix}.tmp"));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            // A failed create_new must never clean up somebody else's pre-existing file.
            let mut file = options.open(&temporary).map_err(unavailable)?;
            let result = (|| {
                file.write_all(MAGIC).map_err(unavailable)?;
                file.write_all(&snapshot.revision.to_le_bytes())
                    .map_err(unavailable)?;
                file.write_all(&snapshot.bytes).map_err(unavailable)?;
                file.sync_all().map_err(unavailable)?;
                Ok(())
            })();
            drop(file);
            let result = result.and_then(|()| {
                fs::rename(&temporary, self.root.join(format!("{id}.snapshot")))
                    .map_err(unavailable)?;
                #[cfg(unix)]
                File::open(&self.root)
                    .and_then(|directory| directory.sync_all())
                    .map_err(unavailable)?;
                Ok(())
            });
            if result.is_err() {
                let _ = fs::remove_file(temporary);
            }
            result
        }
    }
}
