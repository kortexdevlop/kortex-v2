use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Kind, PkgError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Absolute https URL, or a path relative to the index location.
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRelease {
    pub name: BTreeMap<String, String>,
    pub universe: String,
    pub version: String,
    pub targets: BTreeMap<String, Artifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliRelease {
    pub version: String,
    pub targets: BTreeMap<String, Artifact>,
}

fn stable() -> String {
    kortex_catalog::STABLE.to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub schema: u32,
    /// `stable` for public releases, `test` for test feeds (ADR 0034). Absent before ADR 0034.
    #[serde(default = "stable")]
    pub channel: String,
    /// Monotonic publication number shared with the contract catalogue; 0 before ADR 0034.
    #[serde(default)]
    pub sequence: u64,
    /// Commit the published artifacts were built from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_commit: Option<String>,
    pub generated: String,
    #[serde(default)]
    pub cli: Option<CliRelease>,
    pub tools: BTreeMap<String, ToolRelease>,
}

/// `manifest.json` stored inside every package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub target: String,
    pub binary: String,
    pub sha256_binary: String,
}

pub use kortex_catalog::{b64_encode, public_key_b64, sha256_hex, sign_bytes};

/// Verifies `sig_b64` (Ed25519 over the exact bytes of the index) and parses the index, without
/// channel or anti-rollback checks (release tooling). Clients use [`check_index`].
pub fn parse_verified(index_bytes: &[u8], sig_b64: &str, public_key_b64: &str) -> Result<Index> {
    kortex_catalog::verify_signature(index_bytes, sig_b64, public_key_b64).map_err(refused)?;
    parse(index_bytes)
}

/// Client check: signature, channel and floor (the last index accepted on this machine).
/// Returns the index and the new floor to remember.
pub fn check_index(
    index_bytes: &[u8],
    sig_b64: &str,
    public_key_b64: &str,
    channel: &str,
    floor: Option<&kortex_catalog::Floor>,
) -> Result<(Index, kortex_catalog::Floor)> {
    let accepted = kortex_catalog::check(index_bytes, sig_b64, public_key_b64, channel, floor)
        .map_err(refused)?;
    Ok((parse(index_bytes)?, accepted.floor))
}

fn parse(index_bytes: &[u8]) -> Result<Index> {
    let index: Index = serde_json::from_slice(index_bytes)
        .map_err(|e| PkgError::new(Kind::Parse, format!("index illisible : {e}")))?;
    if index.schema != kortex_catalog::SCHEMA {
        return Err(refused(kortex_catalog::Refusal::Schema(index.schema)));
    }
    Ok(index)
}

fn refused(r: kortex_catalog::Refusal) -> PkgError {
    use kortex_catalog::Refusal;
    match r {
        Refusal::BadKey => PkgError::new(Kind::BadSignature, "clé publique invalide"),
        Refusal::BadSignature => PkgError::new(
            Kind::BadSignature,
            "signature de l'index invalide : index non fiable",
        ),
        Refusal::Unreadable(e) => PkgError::new(Kind::Parse, format!("index illisible : {e}")),
        Refusal::Schema(s) => PkgError::new(
            Kind::Unsupported,
            format!("schéma d'index {s} non supporté : mets Kortex à jour"),
        ),
        Refusal::Channel { expected, found } => PkgError::new(
            Kind::BadSignature,
            format!("index du canal {found} refusé (canal attendu : {expected})"),
        ),
        Refusal::Rollback { floor, found } => PkgError::new(
            Kind::Integrity,
            format!(
                "index plus ancien que celui déjà accepté (séquence {found} < {floor}) : retour en arrière refusé"
            ),
        ),
    }
}
