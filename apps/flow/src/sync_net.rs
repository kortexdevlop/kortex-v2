//! The account's sync store as a [`Transport`], and the one place that runs a sync for the UI.
use std::cell::RefCell;

use kortex_ai_client::{Account, Auth, Failure, request};
use kortex_vault::{ROUNDS, Vault};
use serde_json::{Value as Json, json};

use crate::library::Library;
use crate::sync::{Item, Job, Put, Remote, Report, State, SyncError, Transport, sync};

struct Net(Auth);

fn failed(f: Failure) -> SyncError {
    match f {
        Failure::SignedOut => SyncError::Unauthenticated,
        Failure::Network => SyncError::Network,
    }
}

fn code_of(body: &str) -> String {
    serde_json::from_str::<Json>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Json::as_str).map(str::to_owned))
        .unwrap_or_else(|| "server".to_owned())
}

fn item_of(value: &Json) -> Option<Item> {
    Some(Item {
        version: value.get("version")?.as_u64()?,
        data: value.get("data")?.clone(),
    })
}

impl Transport for Net {
    async fn list(&self) -> Result<Vec<Remote>, SyncError> {
        let (status, body) = request(&self.0, "GET", "/v1/sync", None)
            .await
            .map_err(failed)?;
        if status != 200 {
            return Err(SyncError::Server(code_of(&body)));
        }
        let value: Json = serde_json::from_str(&body).map_err(|_| SyncError::Corrupt)?;
        Ok(value
            .get("items")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(|i| {
                let key = i.get("key")?.as_str()?;
                key.starts_with("flow.").then(|| Remote {
                    key: key.to_owned(),
                    version: i.get("version").and_then(Json::as_u64).unwrap_or(0),
                })
            })
            .collect())
    }

    async fn get(&self, key: &str) -> Result<Option<Item>, SyncError> {
        let (status, body) = request(&self.0, "GET", &format!("/v1/sync/{key}"), None)
            .await
            .map_err(failed)?;
        match status {
            404 => Ok(None),
            200 => item_of(&serde_json::from_str(&body).map_err(|_| SyncError::Corrupt)?)
                .map(Some)
                .ok_or(SyncError::Corrupt),
            _ => Err(SyncError::Server(code_of(&body))),
        }
    }

    async fn put(&self, key: &str, data: Json, expected: u64) -> Result<Put, SyncError> {
        let payload = json!({"data": data, "version": expected}).to_string();
        let (status, body) = request(&self.0, "PUT", &format!("/v1/sync/{key}"), Some(&payload))
            .await
            .map_err(failed)?;
        let value: Json = serde_json::from_str(&body).unwrap_or(Json::Null);
        match status {
            200 | 201 => Ok(Put::Done(
                value
                    .get("version")
                    .and_then(Json::as_u64)
                    .unwrap_or(expected + 1),
            )),
            409 => Ok(Put::Conflict(value.get("current").and_then(item_of))),
            _ => Err(SyncError::Server(code_of(&body))),
        }
    }
}

thread_local! {
    /// The derived key, kept for the session so a sync does not repeat the slow derivation.
    static VAULT: RefCell<Option<(String, Vault)>> = const { RefCell::new(None) };
}

/// Forgets the derived key (when sync is turned off).
pub fn forget_key() {
    VAULT.with(|v| *v.borrow_mut() = None);
}

fn random(bytes: &mut [u8]) -> Result<(), SyncError> {
    kortex_platform::fill_random(bytes).map_err(|_| SyncError::Server("random_unavailable".into()))
}

/// Runs a sync and merges the result into `library`; the caller keeps the returned state.
///
/// # Errors
/// `signed_out` when there is no account session, otherwise the sync's own error.
pub async fn run(
    passphrase: &str,
    library: &mut Library,
    state: &mut State,
) -> Result<Report, SyncError> {
    let Account::Ready { auth, .. } = kortex_ai_client::account().await else {
        return Err(SyncError::Unauthenticated);
    };
    let mut cache = VAULT.with(|v| v.borrow_mut().take());
    let now = kortex_platform::now_ms();
    let mut rng = random;
    let job = Job {
        passphrase,
        rounds: ROUNDS,
        now,
        random: &mut rng,
    };
    let result = sync(&Net(auth), job, &mut cache, library, state).await;
    VAULT.with(|v| *v.borrow_mut() = cache);
    result
}
