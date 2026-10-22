//! Project commands shared by UI, Flow and Mind hosts; pure by default.
//! Optional persistence uses the platform. No codecs, resource bytes or OS task supervision.
use std::collections::BTreeMap;

use kortex_contract::v2::Operation;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod jobs;
#[cfg(feature = "persistence")]
pub mod persistence;

/// Local host-issued opaque token. A token is neither a path nor an authorization.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ResourceId(String);

impl TryFrom<String> for ResourceId {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(Error::InvalidResource);
        }
        Ok(Self(value))
    }
}
impl From<ResourceId> for String {
    fn from(value: ResourceId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub id: ResourceId,
    pub mime: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema: u32,
    pub id: String,
    pub revision: u64,
    pub properties: BTreeMap<String, Value>,
    pub resources: BTreeMap<ResourceId, Resource>,
}

impl Project {
    pub fn new(id: String) -> Self {
        Self {
            schema: 1,
            id,
            revision: 0,
            properties: BTreeMap::new(),
            resources: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Ui,
    Flow,
    Mind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    SetProperty { key: String, value: Value },
    RemoveProperty { key: String },
    AttachResource { resource: Resource },
    DetachResource { id: ResourceId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub id: String,
    pub project_id: String,
    pub expected_revision: u64,
    pub operation: String,
    pub source: Source,
    pub changes: Vec<Change>,
}

/// Created by a trusted host after its consent UI; never deserialize this from a model.
#[derive(Debug, Clone, Default)]
pub struct Grants {
    pub network: bool,
    pub filesystem_read: bool,
    pub filesystem_write: bool,
    pub clipboard_read: bool,
    pub clipboard_write: bool,
    pub sensitive: bool,
    pub ai_content: bool,
    pub resources: Vec<ResourceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    UnsupportedSchema,
    RevisionConflict,
    InvalidCommand,
    WrongOperation,
    PermissionDenied,
    InvalidResource,
    InvalidProperty,
    MetadataLimit,
    HistoryEmpty,
    RevisionExhausted,
    DuplicateCommand,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub revision: u64,
    pub replayed: bool,
}

#[derive(Clone)]
struct Entry {
    before: Project,
    after: Project,
    command: Command,
}

/// History is metadata-only and bounded. Callers persist snapshots through kortex-platform.
#[derive(Clone)]
pub struct Session {
    project: Project,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    // Bounded retry window; persisted deduplication is a future host responsibility.
    receipts: Vec<(Command, u64)>,
    max_history: usize,
    max_metadata_bytes: usize,
}

impl Session {
    pub fn open(
        project: Project,
        max_history: usize,
        max_metadata_bytes: usize,
    ) -> Result<Self, Error> {
        if project.schema != 1 {
            return Err(Error::UnsupportedSchema);
        }
        if project.id.is_empty() || max_history == 0 {
            return Err(Error::InvalidCommand);
        }
        check_size(&project, max_metadata_bytes)?;
        for (id, resource) in &project.resources {
            if id != &resource.id || !resource.mime.contains('/') {
                return Err(Error::InvalidResource);
            }
        }
        Ok(Self {
            project,
            undo: vec![],
            redo: vec![],
            receipts: vec![],
            max_history,
            max_metadata_bytes,
        })
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn apply(
        &mut self,
        command: Command,
        operation: &Operation,
        grants: &Grants,
    ) -> Result<Receipt, Error> {
        if command.operation != operation.id || !operation.effects.mutates_project {
            return Err(Error::WrongOperation);
        }
        let p = &operation.permissions;
        if !["none", "read", "write"].contains(&p.filesystem.as_str())
            || !["none", "read", "write"].contains(&p.clipboard.as_str())
            || !["local", "ai-consent"].contains(&p.privacy.as_str())
        {
            return Err(Error::PermissionDenied);
        }
        if (p.network && !grants.network)
            || (p.filesystem == "read" && !grants.filesystem_read)
            || (p.filesystem == "write" && !grants.filesystem_write)
            || (p.clipboard == "read" && !grants.clipboard_read)
            || (p.clipboard == "write" && !grants.clipboard_write)
            || (p.sensitive && !grants.sensitive)
            || (p.privacy == "ai-consent" && !grants.ai_content)
        {
            return Err(Error::PermissionDenied);
        }
        if let Some((previous, revision)) = self.receipts.iter().find(|(c, _)| c.id == command.id) {
            return if previous == &command {
                Ok(Receipt {
                    revision: *revision,
                    replayed: true,
                })
            } else {
                Err(Error::DuplicateCommand)
            };
        }
        if command.id.is_empty()
            || command.project_id != self.project.id
            || command.changes.is_empty()
        {
            return Err(Error::InvalidCommand);
        }
        if command.expected_revision != self.project.revision {
            return Err(Error::RevisionConflict);
        }
        let mut candidate = self.project.clone();
        for change in &command.changes {
            match change {
                Change::SetProperty { key, value } => {
                    check_key(key)?;
                    candidate.properties.insert(key.clone(), value.clone());
                }
                Change::RemoveProperty { key } => {
                    check_key(key)?;
                    candidate.properties.remove(key);
                }
                Change::AttachResource { resource } => {
                    if !grants.resources.contains(&resource.id) {
                        return Err(Error::PermissionDenied);
                    }
                    if !resource.mime.contains('/') {
                        return Err(Error::InvalidResource);
                    }
                    candidate
                        .resources
                        .insert(resource.id.clone(), resource.clone());
                }
                Change::DetachResource { id } => {
                    candidate.resources.remove(id);
                }
            }
            check_size(&candidate, self.max_metadata_bytes)?;
        }
        candidate.revision = next_revision(self.project.revision)?;
        check_size(&candidate, self.max_metadata_bytes)?;
        if operation.effects.undoable {
            self.undo.push(Entry {
                before: self.project.clone(),
                after: candidate.clone(),
                command: command.clone(),
            });
            trim(&mut self.undo, self.max_history);
        } else {
            // Crossing a non-undoable command must not permit restoring an older snapshot.
            self.undo.clear();
        }
        self.redo.clear();
        self.project = candidate;
        self.receipts.push((command, self.project.revision));
        trim(&mut self.receipts, self.max_history);
        Ok(Receipt {
            revision: self.project.revision,
            replayed: false,
        })
    }

    pub fn undo(&mut self, expected_revision: u64) -> Result<(), Error> {
        if expected_revision != self.project.revision {
            return Err(Error::RevisionConflict);
        }
        let revision = next_revision(self.project.revision)?;
        let entry = self.undo.pop().ok_or(Error::HistoryEmpty)?;
        self.project = entry.before.clone();
        self.project.revision = revision;
        self.redo.push(entry);
        Ok(())
    }

    pub fn redo(&mut self, expected_revision: u64) -> Result<(), Error> {
        if expected_revision != self.project.revision {
            return Err(Error::RevisionConflict);
        }
        let revision = next_revision(self.project.revision)?;
        let entry = self.redo.pop().ok_or(Error::HistoryEmpty)?;
        self.project = entry.after.clone();
        self.project.revision = revision;
        self.undo.push(entry);
        Ok(())
    }

    pub fn last_source(&self) -> Option<Source> {
        self.undo.last().map(|entry| entry.command.source)
    }
}

fn next_revision(revision: u64) -> Result<u64, Error> {
    revision.checked_add(1).ok_or(Error::RevisionExhausted)
}
fn check_key(key: &str) -> Result<(), Error> {
    if key.is_empty() || key.len() > 128 {
        return Err(Error::InvalidProperty);
    }
    Ok(())
}
fn check_size(project: &Project, limit: usize) -> Result<(), Error> {
    if serde_json::to_vec(project)
        .map_err(|_| Error::InvalidProperty)?
        .len()
        > limit
    {
        return Err(Error::MetadataLimit);
    }
    Ok(())
}
fn trim<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        items.remove(0);
    }
}
