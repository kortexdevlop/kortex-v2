//! The tool catalogue at runtime (ADR 0034): the signed `catalog.json` published next to the site,
//! else its last verified copy, else the contracts compiled into the app. A document is used only
//! after its Ed25519 signature, channel and anti-rollback floor are checked — never on trust.
use std::ops::Deref;
use std::sync::{Arc, OnceLock, RwLock};

use kortex_catalog::{Floor, Refusal};
use serde::Deserialize;

use crate::Entry;

/// Where the catalogue in use comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Contracts compiled into this build (no verified document available).
    Embedded,
    /// Last verified document kept on this device (offline or network failure).
    Cached,
    /// Verified document just fetched.
    Network,
}

/// A snapshot of the catalogue; cheap to clone.
#[derive(Clone)]
pub struct Catalogue {
    entries: Arc<Vec<Entry>>,
    /// The verified (or embedded) document text, for consumers that read full contracts.
    raw: Arc<str>,
    pub source: Source,
    pub sequence: u64,
}

impl Deref for Catalogue {
    type Target = [Entry];
    fn deref(&self) -> &[Entry] {
        &self.entries
    }
}

impl Catalogue {
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.tool.id == id)
    }

    /// Full contracts (`ToolContract`: limits and permissions included) of the readable tools.
    #[must_use]
    pub fn contracts(&self) -> Vec<kortex_contract::ToolContract> {
        serde_json::from_str::<Document>(&self.raw)
            .map(|d| {
                d.tools
                    .into_iter()
                    .filter_map(|v| serde_json::from_value(v).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// True when both snapshots are the same catalogue (cheap pointer comparison).
    #[must_use]
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.raw, &other.raw)
    }
}

#[derive(Deserialize)]
struct Document {
    tools: Vec<serde_json::Value>,
}

/// Entries this build can read. A contract using fields added after this build is skipped
/// instead of making the whole (verified) catalogue unusable.
fn entries(document: Document) -> Vec<Entry> {
    document
        .tools
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect()
}

/// Contracts compiled into this build (`registry/contracts.json`).
#[must_use]
pub fn embedded() -> Catalogue {
    const TEXT: &str = include_str!("../../../registry/contracts.json");
    static ITEMS: OnceLock<(Arc<Vec<Entry>>, Arc<str>)> = OnceLock::new();
    let (entries, raw) = ITEMS.get_or_init(|| {
        (
            Arc::new(
                serde_json::from_str::<Document>(TEXT)
                    .map(entries)
                    .unwrap_or_default(),
            ),
            Arc::from(TEXT),
        )
    });
    Catalogue {
        entries: Arc::clone(entries),
        raw: Arc::clone(raw),
        source: Source::Embedded,
        sequence: 0,
    }
}

/// Checks a published catalogue against the release key, the channel and the floor.
///
/// # Errors
/// A [`Refusal`]: the document is not used.
pub fn verify(
    bytes: &[u8],
    sig: &str,
    public_key: &str,
    channel: &str,
    floor: Option<&Floor>,
) -> Result<(Vec<Entry>, Floor), Refusal> {
    let accepted = kortex_catalog::check(bytes, sig, public_key, channel, floor)?;
    let document: Document =
        serde_json::from_slice(bytes).map_err(|e| Refusal::Unreadable(e.to_string()))?;
    let tools = entries(document);
    if tools.is_empty() {
        return Err(Refusal::Unreadable("no tools".into()));
    }
    Ok((tools, accepted.floor))
}

/// Channel followed by this client: `stable`, unless a test build or `$KORTEX_CHANNEL` (desktop)
/// asks for `test`.
#[must_use]
pub fn channel() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(c) = kortex_platform::native::env::var("KORTEX_CHANNEL") {
        return c;
    }
    option_env!("KORTEX_CHANNEL")
        .unwrap_or(kortex_catalog::STABLE)
        .to_owned()
}

/// URL of the published catalogue (`$KORTEX_CATALOG` on desktop for a test feed).
#[must_use]
pub fn url() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(u) = kortex_platform::native::env::var("KORTEX_CATALOG") {
        return u;
    }
    if let Some(u) = option_env!("KORTEX_CATALOG") {
        return u.to_owned();
    }
    let root = crate::domain("root");
    let local = root.starts_with("localhost")
        || root
            .split(':')
            .next()
            .is_some_and(|h| h.ends_with(".localhost"));
    format!(
        "{}://{root}/registry/catalog.json",
        if local { "http" } else { "https" }
    )
}

fn names(channel: &str) -> (String, String, String) {
    (
        format!("catalog-{channel}.json"),
        format!("catalog-{channel}.json.sig"),
        format!("catalog-{channel}.floor.json"),
    )
}

fn floor(channel: &str) -> Option<Floor> {
    let (_, _, floor) = names(channel);
    serde_json::from_str(&kortex_platform::public::cache_get(&floor)?).ok()
}

/// The last verified catalogue kept on this device, verified again.
#[must_use]
pub fn cached() -> Option<Catalogue> {
    let channel = channel();
    let (doc, sig, _) = names(&channel);
    let text = kortex_platform::public::cache_get(&doc)?;
    let sig = kortex_platform::public::cache_get(&sig)?;
    let floor = floor(&channel);
    let (tools, accepted) = verify(
        text.as_bytes(),
        &sig,
        kortex_catalog::RELEASE_PUBLIC_KEY,
        &channel,
        floor.as_ref(),
    )
    .ok()?;
    Some(Catalogue {
        entries: Arc::new(tools),
        raw: Arc::from(text),
        source: Source::Cached,
        sequence: accepted.sequence,
    })
}

/// Fetches and verifies the published catalogue, then keeps it for offline use.
/// `None` when the network fails or the document is refused: the current catalogue stays.
pub async fn fetch() -> Option<Catalogue> {
    let channel = channel();
    let url = url();
    let text = kortex_platform::public::fetch_text(&url, kortex_platform::public::MAX_BYTES)
        .await
        .ok()?;
    let sig = kortex_platform::public::fetch_text(&format!("{url}.sig"), 4096)
        .await
        .ok()?;
    let previous = floor(&channel);
    let (tools, accepted) = verify(
        text.as_bytes(),
        &sig,
        kortex_catalog::RELEASE_PUBLIC_KEY,
        &channel,
        previous.as_ref(),
    )
    .ok()?;
    let (doc, sig_name, floor_name) = names(&channel);
    // The floor is written last: a partial write leaves the previous, still valid, state.
    let stored = kortex_platform::public::cache_put(&doc, &text)
        .and_then(|()| kortex_platform::public::cache_put(&sig_name, &sig))
        .and_then(|()| {
            kortex_platform::public::cache_put(
                &floor_name,
                &serde_json::to_string(&accepted).unwrap_or_default(),
            )
        });
    let _ = stored; // offline copy is best effort; the verified document is used either way
    Some(Catalogue {
        sequence: accepted.sequence,
        entries: Arc::new(tools),
        raw: Arc::from(text),
        source: Source::Network,
    })
}

fn slot() -> &'static RwLock<Catalogue> {
    static CURRENT: OnceLock<RwLock<Catalogue>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(cached().unwrap_or_else(embedded)))
}

/// The catalogue in use (initially the cached verified copy, else the embedded one).
#[must_use]
pub fn current() -> Catalogue {
    slot()
        .read()
        .map_or_else(|e| e.into_inner().clone(), |c| c.clone())
}

/// Replaces the catalogue in use, unless it is older than the one already in use.
pub(crate) fn replace(next: Catalogue) -> bool {
    let mut guard = slot()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if guard.source != Source::Embedded && next.sequence < guard.sequence {
        return false;
    }
    *guard = next;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; 32] = [9; 32];

    fn signed(sequence: u64, channel: &str, tools: &str) -> (Vec<u8>, String) {
        let bytes = format!(
            r#"{{"schema":1,"channel":"{channel}","sequence":{sequence},"generated":"2026-10-20","tools":{tools}}}"#
        )
        .into_bytes();
        let sig = kortex_catalog::sign_bytes(&SEED, &bytes);
        (bytes, sig)
    }

    fn embedded_tools() -> String {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../../../registry/contracts.json"))
                .unwrap_or_default();
        value["tools"].to_string()
    }

    #[test]
    fn the_embedded_catalogue_is_the_registry_of_this_build() {
        let registry: serde_json::Value =
            serde_json::from_str(include_str!("../../../registry/contracts.json"))
                .unwrap_or_default();
        let expected = registry["tools"].as_array().map_or(0, Vec::len);
        let embedded = embedded();
        assert!(expected > 0);
        assert_eq!(embedded.len(), expected, "every contract of the registry");
        assert_eq!(embedded.source, Source::Embedded);
        assert!(embedded.find("secure.hash-calculator").is_some());
    }

    #[test]
    fn a_signed_catalogue_with_a_new_tool_is_used() {
        let mut tools: Vec<serde_json::Value> =
            serde_json::from_str(&embedded_tools()).unwrap_or_default();
        let mut extra = tools[0].clone();
        extra["tool"]["id"] = "image.compress".into();
        tools.push(extra);
        // A contract field unknown to this build only skips that entry.
        let mut future = tools[0].clone();
        future["tool"]["id"] = "code.future".into();
        future["tool"]["field_from_a_later_schema"] = true.into();
        tools.push(future);
        let (bytes, sig) = signed(
            3,
            "stable",
            &serde_json::to_string(&tools).unwrap_or_default(),
        );
        let key = kortex_catalog::public_key_b64(&SEED);
        let (entries, floor) =
            verify(&bytes, &sig, &key, "stable", None).unwrap_or_else(|e| unreachable!("{e}"));
        assert!(entries.iter().any(|e| e.tool.id == "image.compress"));
        assert!(!entries.iter().any(|e| e.tool.id == "code.future"));
        assert_eq!(entries.len(), tools.len() - 1);
        assert_eq!(floor.sequence, 3);
    }

    #[test]
    fn forged_replayed_or_foreign_catalogues_are_refused() {
        let key = kortex_catalog::public_key_b64(&SEED);
        let (bytes, sig) = signed(5, "stable", &embedded_tools());
        let (_, floor) =
            verify(&bytes, &sig, &key, "stable", None).unwrap_or_else(|e| unreachable!("{e}"));
        let mut forged = bytes.clone();
        let at = forged.len() / 2;
        forged[at] ^= 1;
        assert!(verify(&forged, &sig, &key, "stable", None).is_err());
        assert!(
            verify(
                &bytes,
                &sig,
                kortex_catalog::RELEASE_PUBLIC_KEY,
                "stable",
                None
            )
            .is_err()
        );
        let (old, old_sig) = signed(4, "stable", &embedded_tools());
        assert!(matches!(
            verify(&old, &old_sig, &key, "stable", Some(&floor)),
            Err(Refusal::Rollback { .. })
        ));
        let (test, test_sig) = signed(6, "test", &embedded_tools());
        assert!(matches!(
            verify(&test, &test_sig, &key, "stable", Some(&floor)),
            Err(Refusal::Channel { .. })
        ));
        let (empty, empty_sig) = signed(7, "stable", "[]");
        assert!(verify(&empty, &empty_sig, &key, "stable", Some(&floor)).is_err());
    }
}
