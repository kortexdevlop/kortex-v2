//! Run history, pure: what happened in each run (which card finished, was skipped or failed, how
//! long it took, the error code) and nothing the cards produced or received, so no data of the
//! user is kept. Stored on this device, newest first.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Runs kept.
pub const KEEP: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRun {
    pub node: String,
    pub title: String,
    /// `done`, `skipped`, `failed` or `not_run`.
    pub state: String,
    pub ms: u64,
    /// Error code (and detail) of a failed card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: u64,
    pub workflow: String,
    /// Start, milliseconds since the epoch.
    pub at: u64,
    pub ok: bool,
    pub total_ms: u64,
    pub nodes: Vec<NodeRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Runs {
    #[serde(default)]
    pub items: Vec<RunRecord>,
}

impl Runs {
    pub fn push(&mut self, record: RunRecord) {
        self.items.insert(0, record);
        self.items.truncate(KEEP);
    }

    pub fn remove(&mut self, id: u64) {
        self.items.retain(|r| r.id != id);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// What a run looked like when it ended.
pub struct Outcome<'a> {
    pub workflow: &'a str,
    pub at: u64,
    pub total_ms: u64,
    /// Cards in execution order with their titles.
    pub order: &'a [(String, String)],
    /// Time spent in each card that ran a job.
    pub ms: &'a BTreeMap<String, u64>,
    pub done: &'a BTreeSet<String>,
    pub skipped: &'a BTreeSet<String>,
    /// The card that stopped the run and its error.
    pub failed: Option<(&'a str, String)>,
}

/// Builds the record of a run.
#[must_use]
pub fn record(outcome: &Outcome<'_>) -> RunRecord {
    let nodes = outcome
        .order
        .iter()
        .map(|(node, title)| {
            let failed = outcome.failed.as_ref().filter(|(id, _)| id == node);
            let state = if failed.is_some() {
                "failed"
            } else if outcome.done.contains(node) {
                "done"
            } else if outcome.skipped.contains(node) {
                "skipped"
            } else {
                "not_run"
            };
            NodeRun {
                node: node.clone(),
                title: title.clone(),
                state: state.to_owned(),
                ms: outcome.ms.get(node).copied().unwrap_or(0),
                error: failed.map(|(_, e)| e.chars().take(120).collect()),
            }
        })
        .collect();
    RunRecord {
        id: outcome.at,
        workflow: outcome.workflow.to_owned(),
        at: outcome.at,
        ok: outcome.failed.is_none(),
        total_ms: outcome.total_ms,
        nodes,
    }
}

/// `2026-10-19 21:40 UTC` from milliseconds since the epoch.
#[must_use]
pub fn utc_label(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60
    )
}

/// `850 ms` or `2.4 s`.
#[must_use]
pub fn duration_label(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        format!("{:.1} s", ms as f64 / 1000.0)
    }
}
