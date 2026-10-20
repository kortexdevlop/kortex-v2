//! The user's saved workflows ("Mes workflows"), pure. Deleting leaves a tombstone so that a
//! later sync between devices can tell "deleted here" from "never seen here".
use serde::{Deserialize, Serialize};

use crate::engine::Workflow;

/// Tombstones older than this are dropped (milliseconds).
const TOMBSTONE_TTL_MS: u64 = 30 * 24 * 3600 * 1000;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Saved {
    pub id: String,
    pub name: String,
    /// Milliseconds since the epoch of the last change.
    pub updated_at: u64,
    #[serde(default)]
    pub deleted: bool,
    pub workflow: Workflow,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Library {
    #[serde(default)]
    pub items: Vec<Saved>,
}

impl Library {
    /// Workflows to show, most recently changed first.
    #[must_use]
    pub fn visible(&self) -> Vec<&Saved> {
        let mut shown: Vec<&Saved> = self.items.iter().filter(|s| !s.deleted).collect();
        shown.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.name.cmp(&b.name)));
        shown
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Saved> {
        self.items.iter().find(|s| s.id == id && !s.deleted)
    }

    /// Saves `workflow` under `id` (replacing it) or under a new id. Returns the id.
    pub fn save(
        &mut self,
        id: Option<&str>,
        name: &str,
        workflow: &Workflow,
        now: u64,
        new_id: impl FnOnce() -> String,
    ) -> String {
        let name = name.trim();
        let entry = Saved {
            id: String::new(),
            name: name.to_owned(),
            updated_at: now,
            deleted: false,
            workflow: workflow.clone(),
        };
        if let Some(existing) = id.and_then(|id| self.items.iter_mut().find(|s| s.id == id)) {
            let kept = existing.id.clone();
            *existing = Saved {
                id: kept.clone(),
                ..entry
            };
            return kept;
        }
        let id = id.map_or_else(new_id, str::to_owned);
        self.items.push(Saved {
            id: id.clone(),
            ..entry
        });
        id
    }

    /// A copy named "<name> (copie)" / "(copy)"; `None` for an unknown id.
    pub fn duplicate(
        &mut self,
        id: &str,
        suffix: &str,
        now: u64,
        new_id: impl FnOnce() -> String,
    ) -> Option<String> {
        let source = self.get(id)?.clone();
        let name = format!("{} {suffix}", source.name);
        Some(self.save(None, &name, &source.workflow, now, new_id))
    }

    pub fn rename(&mut self, id: &str, name: &str, now: u64) {
        if let Some(s) = self.items.iter_mut().find(|s| s.id == id && !s.deleted) {
            s.name = name.trim().to_owned();
            s.workflow.name = s.name.clone();
            s.updated_at = now;
        }
    }

    /// Marks a workflow deleted and empties it; the tombstone travels with a sync.
    pub fn remove(&mut self, id: &str, now: u64) {
        if let Some(s) = self.items.iter_mut().find(|s| s.id == id) {
            s.deleted = true;
            s.workflow = Workflow::default();
            s.updated_at = now;
        }
    }

    /// Merges another copy of the library (another device): the newer change of each workflow
    /// wins, a deletion wins over an older edit. Old tombstones are dropped.
    pub fn merge(&mut self, other: &Self, now: u64) {
        for theirs in &other.items {
            match self.items.iter_mut().find(|s| s.id == theirs.id) {
                Some(mine) if mine.updated_at >= theirs.updated_at => {}
                Some(mine) => *mine = theirs.clone(),
                None => self.items.push(theirs.clone()),
            }
        }
        self.items
            .retain(|s| !(s.deleted && now.saturating_sub(s.updated_at) > TOMBSTONE_TTL_MS));
    }
}

/// A short random id (`w` + 12 hex digits).
#[must_use]
pub fn new_id() -> String {
    let mut bytes = [0_u8; 6];
    if kortex_platform::fill_random(&mut bytes).is_err() {
        bytes = u64::to_le_bytes(kortex_platform::now_ms())[..6]
            .try_into()
            .unwrap_or([0; 6]);
    }
    format!(
        "w{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
