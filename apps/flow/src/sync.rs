//! Sync of the library between devices, end to end encrypted. Each saved workflow is one item
//! (`flow.<id>`) of the account's sync store, sealed with a key derived from a passphrase that stays
//! on the devices; a small `flow.vault` item holds the salt and a check blob so a wrong passphrase
//! is recognised before anything is written. The server only ever sees ciphertext and sizes.
//!
//! The algorithm is transport-agnostic ([`Transport`]) so it is tested against an in-memory server
//! with concurrent writers.
use std::collections::BTreeMap;

use kortex_vault::{Vault, VaultError};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::library::{Library, Saved};

pub const VAULT_KEY: &str = "flow.vault";

/// Sync key of a saved workflow.
#[must_use]
pub fn key_of(id: &str) -> String {
    format!("flow.{id}")
}

/// The workflow id behind a sync key (`None` for the vault item and for other apps' keys).
#[must_use]
pub fn id_of(key: &str) -> Option<&str> {
    key.strip_prefix("flow.").filter(|id| id.starts_with('w'))
}

/// What this device last saw of one workflow on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    pub version: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub seen: BTreeMap<String, Seen>,
    #[serde(default)]
    pub last_sync: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub key: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub version: u64,
    pub data: Json,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Put {
    Done(u64),
    /// Someone wrote first; the current copy is attached.
    Conflict(Option<Item>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    Unauthenticated,
    Network,
    Server(String),
    WrongPassphrase,
    Corrupt,
}

impl SyncError {
    /// Locale key suffix (`sync.<code>`).
    #[must_use]
    pub fn code(&self) -> &str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::Network => "network",
            Self::Server(code) => code,
            Self::WrongPassphrase => "wrong_passphrase",
            Self::Corrupt => "corrupt",
        }
    }
}

/// The server's sync store.
pub trait Transport {
    fn list(&self) -> impl Future<Output = Result<Vec<Remote>, SyncError>>;
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Item>, SyncError>>;
    fn put(
        &self,
        key: &str,
        data: Json,
        expected: u64,
    ) -> impl Future<Output = Result<Put, SyncError>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Report {
    pub pulled: usize,
    pub pushed: usize,
    pub conflicts: usize,
}

/// Everything a sync needs besides the transport.
pub struct Job<'a> {
    pub passphrase: &'a str,
    /// Key-derivation rounds (the real value is `kortex_vault::ROUNDS`).
    pub rounds: u32,
    pub now: u64,
    /// Fills a buffer with secure random bytes, or fails before any ciphertext is written.
    pub random: &'a mut dyn FnMut(&mut [u8]) -> Result<(), SyncError>,
}

fn nonce(job: &mut Job<'_>) -> Result<[u8; 12], SyncError> {
    let mut bytes = [0_u8; 12];
    (job.random)(&mut bytes)?;
    Ok(bytes)
}

fn vault_error(e: VaultError) -> SyncError {
    match e {
        VaultError::Rejected => SyncError::WrongPassphrase,
        VaultError::Malformed => SyncError::Corrupt,
    }
}

async fn open_vault(
    transport: &impl Transport,
    job: &mut Job<'_>,
    cache: &mut Option<(String, Vault)>,
) -> Result<(), SyncError> {
    for _ in 0..2 {
        match transport.get(VAULT_KEY).await? {
            Some(item) => {
                let salt = item
                    .data
                    .get("salt")
                    .and_then(Json::as_str)
                    .ok_or(SyncError::Corrupt)?
                    .to_owned();
                let check = item
                    .data
                    .get("check")
                    .and_then(Json::as_str)
                    .ok_or(SyncError::Corrupt)?;
                if cache.as_ref().is_none_or(|(s, _)| *s != salt) {
                    let vault = Vault::derive_with(job.passphrase, &salt, job.rounds)
                        .map_err(vault_error)?;
                    *cache = Some((salt, vault));
                }
                let accepted = cache.as_ref().is_some_and(|(_, v)| v.accepts(check));
                return if accepted {
                    Ok(())
                } else {
                    Err(SyncError::WrongPassphrase)
                };
            }
            None => {
                // First device of the account: it creates the vault.
                let mut random_error = None;
                let salt = kortex_vault::new_salt(|b| {
                    random_error = (job.random)(b).err();
                });
                if let Some(error) = random_error {
                    return Err(error);
                }
                let vault =
                    Vault::derive_with(job.passphrase, &salt, job.rounds).map_err(vault_error)?;
                let check = vault.check_blob(nonce(job)?);
                match transport
                    .put(VAULT_KEY, json!({"salt": salt, "check": check}), 0)
                    .await?
                {
                    Put::Done(_) => {
                        *cache = Some((salt, vault));
                        return Ok(());
                    }
                    // Another device created it at the same moment: read theirs.
                    Put::Conflict(_) => {}
                }
            }
        }
    }
    Err(SyncError::Server("vault_conflict".into()))
}

fn read_item(vault: &Vault, item: &Item) -> Result<Saved, SyncError> {
    let blob = item
        .data
        .get("c")
        .and_then(Json::as_str)
        .ok_or(SyncError::Corrupt)?;
    let plain = vault.open(blob).map_err(|e| match e {
        // The vault check passed, so a rejected item is damaged, not a wrong passphrase.
        VaultError::Rejected | VaultError::Malformed => SyncError::Corrupt,
    })?;
    serde_json::from_slice(&plain).map_err(|_| SyncError::Corrupt)
}

fn absorb(library: &mut Library, saved: Saved, now: u64) {
    library.merge(&Library { items: vec![saved] }, now);
}

/// Brings this device and the server to the same library.
///
/// # Errors
/// [`SyncError`]: no session, network, a server refusal, a wrong passphrase, or damaged data.
pub async fn sync(
    transport: &impl Transport,
    mut job: Job<'_>,
    cache: &mut Option<(String, Vault)>,
    library: &mut Library,
    state: &mut State,
) -> Result<Report, SyncError> {
    let mut report = Report::default();
    let remote = transport.list().await?;
    open_vault(transport, &mut job, cache).await?;
    let vault = &cache.as_ref().ok_or(SyncError::Corrupt)?.1;
    let versions: BTreeMap<&str, u64> = remote
        .iter()
        .filter_map(|r| id_of(&r.key).map(|id| (id, r.version)))
        .collect();
    // Pull what is newer on the server.
    for (id, version) in &versions {
        let seen = state.seen.get(*id).map_or(0, |s| s.version);
        if *version <= seen {
            continue;
        }
        if let Some(item) = transport.get(&key_of(id)).await? {
            let saved = read_item(vault, &item)?;
            state.seen.insert(
                (*id).to_owned(),
                Seen {
                    version: item.version,
                    updated_at: saved.updated_at,
                },
            );
            absorb(library, saved, job.now);
            report.pulled += 1;
        }
    }
    // Push what is newer here.
    let local: Vec<Saved> = library.items.clone();
    for item in local {
        let seen = state.seen.get(&item.id).copied();
        if seen.is_some_and(|s| item.updated_at <= s.updated_at) {
            continue;
        }
        let mut expected = seen.map_or_else(
            || versions.get(item.id.as_str()).copied().unwrap_or(0),
            |s| s.version,
        );
        let mut current = item;
        for attempt in 0..2 {
            let plain = serde_json::to_vec(&current).map_err(|_| SyncError::Corrupt)?;
            let data = json!({"c": vault.seal(&plain, nonce(&mut job)?)});
            match transport.put(&key_of(&current.id), data, expected).await? {
                Put::Done(version) => {
                    state.seen.insert(
                        current.id.clone(),
                        Seen {
                            version,
                            updated_at: current.updated_at,
                        },
                    );
                    report.pushed += 1;
                    break;
                }
                Put::Conflict(found) => {
                    report.conflicts += 1;
                    let Some(theirs) = found else { break };
                    let remote_saved = read_item(vault, &theirs)?;
                    let their_updated = remote_saved.updated_at;
                    absorb(library, remote_saved, job.now);
                    let merged = library
                        .items
                        .iter()
                        .find(|s| s.id == current.id)
                        .cloned()
                        .unwrap_or(current.clone());
                    state.seen.insert(
                        current.id.clone(),
                        Seen {
                            version: theirs.version,
                            updated_at: their_updated,
                        },
                    );
                    // Theirs may have been newer (nothing to push) or ours (push once more).
                    if merged.updated_at <= their_updated || attempt == 1 {
                        break;
                    }
                    expected = theirs.version;
                    current = merged;
                }
            }
        }
    }
    state.last_sync = job.now;
    Ok(report)
}
