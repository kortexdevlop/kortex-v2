use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub schema: u32,
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

pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0_u32, 0_u32);
    for c in s.trim().bytes().filter(|c| *c != b'=') {
        let v = B64.iter().position(|x| *x == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

pub(crate) fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |a, (i, b)| a | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                char::from(B64[((n >> (18 - 6 * i)) & 63) as usize])
            } else {
                '='
            });
        }
    }
    out
}

/// Verifies `sig_b64` (Ed25519 over the exact bytes of the index) and parses the index.
pub fn parse_verified(index_bytes: &[u8], sig_b64: &str, public_key_b64: &str) -> Result<Index> {
    let bad = |m: &str| PkgError::new(Kind::BadSignature, m.to_owned());
    let key: [u8; 32] = b64_decode(public_key_b64)
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| bad("clé publique invalide"))?;
    let sig: [u8; 64] = b64_decode(sig_b64)
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| bad("signature illisible"))?;
    let vk = VerifyingKey::from_bytes(&key).map_err(|_| bad("clé publique invalide"))?;
    vk.verify_strict(index_bytes, &Signature::from_bytes(&sig))
        .map_err(|_| bad("signature de l'index invalide : index non fiable"))?;
    let index: Index = serde_json::from_slice(index_bytes)
        .map_err(|e| PkgError::new(Kind::Parse, format!("index illisible : {e}")))?;
    if index.schema != 1 {
        return Err(PkgError::new(
            Kind::Unsupported,
            format!(
                "schéma d'index {} non supporté : mets Kortex à jour",
                index.schema
            ),
        ));
    }
    Ok(index)
}

/// Public key (base64) matching a seed; used to publish the key embedded in clients.
pub fn public_key_b64(seed: &[u8; 32]) -> String {
    b64_encode(&SigningKey::from_bytes(seed).verifying_key().to_bytes())
}

/// Signs `bytes` with a 32-byte Ed25519 seed (release tooling only). Returns base64.
pub fn sign_bytes(seed: &[u8; 32], bytes: &[u8]) -> String {
    b64_encode(&SigningKey::from_bytes(seed).sign(bytes).to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        for n in 0..40 {
            let data: Vec<u8> = (0..n).map(|i| (i * 7 + 3) as u8).collect();
            assert_eq!(b64_decode(&b64_encode(&data)), Some(data));
        }
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
        assert!(b64_decode("not base64!").is_none());
    }

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
