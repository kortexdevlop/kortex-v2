//! Automatic triggers of saved workflows, pure: when is a trigger due, when does it run next, and
//! what a run changes (failures switch a trigger off). Triggers are local to a device: a workflow
//! synced to another device does not start running there on its own. They only run while Kortex
//! Flow is open (a browser tab or the desktop app): there is no background service.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const MINUTE: u64 = 60_000;
const DAY: u64 = 24 * 60 * MINUTE;
/// Consecutive failures after which a trigger switches itself off.
pub const MAX_FAILURES: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// Every `minutes` minutes.
    Every { minutes: u32 },
    /// Every day at a local time.
    Daily { hour: u8, minute: u8 },
    /// Once each time Flow is opened.
    Open,
    /// When a local program calls `POST http://127.0.0.1:<port>/hook/<token>` (desktop only).
    Webhook { port: u16, token: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Last {
    pub at: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub kind: Kind,
    pub enabled: bool,
    /// Last start, milliseconds since the epoch (0 = never).
    #[serde(default)]
    pub last_run: u64,
    #[serde(default)]
    pub failures: u8,
    #[serde(default)]
    pub last: Option<Last>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Triggers {
    #[serde(default)]
    pub items: BTreeMap<String, Config>,
}

/// Shortest interval allowed: a background browser tab is throttled anyway, and a workflow that
/// calls the AI spends shared credits on every run.
#[must_use]
pub fn min_every(has_ai: bool, desktop: bool) -> u32 {
    if has_ai {
        15
    } else if desktop {
        1
    } else {
        5
    }
}

/// Why a trigger cannot be saved as it is (`trigger.err_<code>` in the locales).
///
/// # Errors
/// `interval_too_short`, `bad_time`, or `webhook_web`.
pub fn check(kind: &Kind, has_ai: bool, desktop: bool) -> Result<(), &'static str> {
    match kind {
        Kind::Every { minutes } if *minutes < min_every(has_ai, desktop) || *minutes > 10_080 => {
            Err("interval_too_short")
        }
        Kind::Daily { hour, minute } if *hour > 23 || *minute > 59 => Err("bad_time"),
        Kind::Webhook { .. } if !desktop => Err("webhook_web"),
        _ => Ok(()),
    }
}

impl Config {
    #[must_use]
    pub fn new(kind: Kind, now: u64) -> Self {
        // An interval starts counting when it is switched on (no immediate surprise run).
        Self {
            kind,
            enabled: true,
            last_run: now,
            failures: 0,
            last: None,
        }
    }

    /// The most recent scheduled instant of a daily trigger that is not in the future.
    fn latest_slot(hour: u8, minute: u8, now: u64, offset_minutes: i32) -> u64 {
        let offset = i64::from(offset_minutes) * MINUTE as i64;
        let local_now = now as i64 + offset;
        let day_start = local_now - local_now.rem_euclid(DAY as i64);
        let mut slot = day_start + (i64::from(hour) * 60 + i64::from(minute)) * MINUTE as i64;
        if slot > local_now {
            slot -= DAY as i64;
        }
        u64::try_from(slot - offset).unwrap_or(0)
    }

    /// Whether the trigger should start a run now. `opened` is whether this session already ran the
    /// "when opened" triggers; webhooks are event-driven and never due by the clock.
    #[must_use]
    pub fn due(&self, now: u64, offset_minutes: i32, opened: bool) -> bool {
        if !self.enabled || self.failures >= MAX_FAILURES {
            return false;
        }
        match &self.kind {
            Kind::Every { minutes } => {
                now.saturating_sub(self.last_run) >= u64::from(*minutes) * MINUTE
            }
            Kind::Daily { hour, minute } => {
                self.last_run < Self::latest_slot(*hour, *minute, now, offset_minutes)
            }
            Kind::Open => !opened,
            Kind::Webhook { .. } => false,
        }
    }

    /// When it will next run by the clock (`None` for event-driven triggers and switched-off ones).
    #[must_use]
    pub fn next_run(&self, now: u64, offset_minutes: i32) -> Option<u64> {
        if !self.enabled || self.failures >= MAX_FAILURES {
            return None;
        }
        match &self.kind {
            Kind::Every { minutes } => {
                Some((self.last_run + u64::from(*minutes) * MINUTE).max(now))
            }
            Kind::Daily { hour, minute } => {
                let latest = Self::latest_slot(*hour, *minute, now, offset_minutes);
                // Overdue (missed while closed) means "at the next opportunity", otherwise tomorrow.
                Some(if self.last_run < latest {
                    now
                } else {
                    latest + DAY
                })
            }
            Kind::Open | Kind::Webhook { .. } => None,
        }
    }

    /// Records a finished run; five failures in a row switch the trigger off.
    pub fn finished(&mut self, started: u64, ok: bool, error: Option<String>) {
        self.last_run = started;
        self.last = Some(Last {
            at: started,
            ok,
            error,
        });
        if ok {
            self.failures = 0;
        } else {
            self.failures = self.failures.saturating_add(1);
            if self.failures >= MAX_FAILURES {
                self.enabled = false;
            }
        }
    }

    #[must_use]
    pub fn webhook_url(&self) -> Option<String> {
        match &self.kind {
            Kind::Webhook { port, token } => Some(format!("http://127.0.0.1:{port}/hook/{token}")),
            _ => None,
        }
    }
}
