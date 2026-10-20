//! Signed Kortex documents: the package index (`index.json`) and the contract catalogue
//! (`catalog.json`), each with a detached Ed25519 signature over its exact bytes (ADR 0007, 0034).
//!
//! A document is accepted only if it is signed by the release key, has a supported schema, belongs
//! to the client's channel and is not older than the last document the client accepted (its
//! *floor*). Pure: no network, no storage; hosts fetch and keep the floor through `kortex-platform`.
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Public key of the release signing key (Ed25519, raw 32 bytes, base64), compiled into clients.
/// Regenerate with `cargo xtask keygen`; the private half never enters the repository.
pub const RELEASE_PUBLIC_KEY: &str = include_str!("../release.pub");

/// Schema of the signed documents this client understands.
pub const SCHEMA: u32 = 1;
/// Channel of public releases; `test` feeds are refused unless explicitly asked for.
pub const STABLE: &str = "stable";

/// Why a signed document was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The public key compiled into the client is unusable.
    BadKey,
    /// Signature missing, malformed or not made by the release key.
    BadSignature,
    /// Signed, but not a readable document.
    Unreadable(String),
    /// A schema this client does not know.
    Schema(u32),
    /// A document of another channel (e.g. a `test` feed served to a stable client).
    Channel { expected: String, found: String },
    /// Older than (or different from, at the same sequence) the last accepted document.
    Rollback { floor: u64, found: u64 },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadKey => f.write_str("invalid release public key"),
            Self::BadSignature => f.write_str("invalid signature: document not trusted"),
            Self::Unreadable(e) => write!(f, "unreadable document: {e}"),
            Self::Schema(s) => write!(f, "unsupported schema {s}: update Kortex"),
            Self::Channel { expected, found } => {
                write!(f, "document of channel {found}, expected {expected}")
            }
            Self::Rollback { floor, found } => {
                write!(f, "sequence {found} is older than the accepted {floor}")
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// The last accepted document of one kind and channel. Anything older is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Floor {
    pub sequence: u64,
    pub sha256: String,
}

/// Header shared by every signed document. Documents published before ADR 0034 have neither
/// `channel` (stable) nor `sequence` (0, never accepted again once a sequenced one was).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Header {
    schema: u32,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    sequence: u64,
}

/// A verified document, with the floor to remember for the next check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    pub channel: String,
    pub sequence: u64,
    pub floor: Floor,
}

/// Verifies `sig_b64` and the header of `bytes` for `channel`, against the previous `floor`.
///
/// # Errors
/// A [`Refusal`]; the caller keeps its cached or embedded document and never uses `bytes`.
pub fn check(
    bytes: &[u8],
    sig_b64: &str,
    public_key_b64: &str,
    channel: &str,
    floor: Option<&Floor>,
) -> Result<Accepted, Refusal> {
    verify_signature(bytes, sig_b64, public_key_b64)?;
    let header: Header =
        serde_json::from_slice(bytes).map_err(|e| Refusal::Unreadable(e.to_string()))?;
    if header.schema != SCHEMA {
        return Err(Refusal::Schema(header.schema));
    }
    let found = header.channel.unwrap_or_else(|| STABLE.to_owned());
    if found != channel {
        return Err(Refusal::Channel {
            expected: channel.to_owned(),
            found,
        });
    }
    let sha256 = sha256_hex(bytes);
    if let Some(floor) = floor {
        let older = header.sequence < floor.sequence;
        // Same sequence: only the very same document. Unsequenced (0) legacy documents may
        // change until the first sequenced one is accepted.
        let replaced =
            header.sequence == floor.sequence && header.sequence > 0 && sha256 != floor.sha256;
        if older || replaced {
            return Err(Refusal::Rollback {
                floor: floor.sequence,
                found: header.sequence,
            });
        }
    }
    Ok(Accepted {
        channel: found,
        sequence: header.sequence,
        floor: Floor {
            sequence: header.sequence,
            sha256,
        },
    })
}

/// Ed25519 (strict) over the exact bytes.
///
/// # Errors
/// [`Refusal::BadKey`] or [`Refusal::BadSignature`].
pub fn verify_signature(bytes: &[u8], sig_b64: &str, public_key_b64: &str) -> Result<(), Refusal> {
    let key: [u8; 32] = b64_decode(public_key_b64)
        .and_then(|v| v.try_into().ok())
        .ok_or(Refusal::BadKey)?;
    let sig: [u8; 64] = b64_decode(sig_b64)
        .and_then(|v| v.try_into().ok())
        .ok_or(Refusal::BadSignature)?;
    let vk = VerifyingKey::from_bytes(&key).map_err(|_| Refusal::BadKey)?;
    vk.verify_strict(bytes, &Signature::from_bytes(&sig))
        .map_err(|_| Refusal::BadSignature)
}

/// Public key (base64) matching a seed; used to publish the key embedded in clients.
#[must_use]
pub fn public_key_b64(seed: &[u8; 32]) -> String {
    b64_encode(&SigningKey::from_bytes(seed).verifying_key().to_bytes())
}

/// Signs `bytes` with a 32-byte Ed25519 seed (release tooling only). Returns base64.
#[must_use]
pub fn sign_bytes(seed: &[u8; 32], bytes: &[u8]) -> String {
    b64_encode(&SigningKey::from_bytes(seed).sign(bytes).to_bytes())
}

#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[must_use]
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0_u32, 0_u32);
    for c in s.trim().bytes().filter(|c| *c != b'=') {
        let v = u32::try_from(B64.iter().position(|x| *x == c)?).ok()?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xFF).ok()?);
        }
    }
    Some(out)
}

#[must_use]
pub fn b64_encode(bytes: &[u8]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; 32] = [7; 32];

    fn doc(channel: Option<&str>, sequence: Option<u64>, body: &str) -> Vec<u8> {
        let mut fields = vec!["\"schema\":1".to_owned()];
        if let Some(c) = channel {
            fields.push(format!("\"channel\":\"{c}\""));
        }
        if let Some(s) = sequence {
            fields.push(format!("\"sequence\":{s}"));
        }
        fields.push(format!("\"tools\":[\"{body}\"]"));
        format!("{{{}}}", fields.join(",")).into_bytes()
    }

    fn accept(bytes: &[u8], floor: Option<&Floor>) -> Result<Accepted, Refusal> {
        check(
            bytes,
            &sign_bytes(&SEED, bytes),
            &public_key_b64(&SEED),
            STABLE,
            floor,
        )
    }

    #[test]
    fn base64_roundtrip_and_known_hash() {
        for n in 0..40 {
            let data: Vec<u8> = (0..n).map(|i| (i * 7 + 3) as u8).collect();
            assert_eq!(b64_decode(&b64_encode(&data)), Some(data));
        }
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
        assert!(b64_decode("not base64!").is_none());
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_signed_document_of_the_channel_is_accepted_with_its_floor() {
        let bytes = doc(Some("stable"), Some(4), "a");
        let ok = accept(&bytes, None).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!((ok.sequence, ok.channel.as_str()), (4, "stable"));
        assert_eq!(ok.floor.sha256, sha256_hex(&bytes));
        // Legacy documents (no channel, no sequence) are stable at sequence 0.
        let legacy = accept(&doc(None, None, "a"), None).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!((legacy.sequence, legacy.channel.as_str()), (0, "stable"));
    }

    #[test]
    fn tampered_or_foreign_signatures_are_refused() {
        let bytes = doc(None, Some(1), "a");
        let sig = sign_bytes(&SEED, &bytes);
        let key = public_key_b64(&SEED);
        let mut tampered = bytes.clone();
        let at = tampered.len() - 3;
        tampered[at] ^= 1;
        assert_eq!(
            check(&tampered, &sig, &key, STABLE, None),
            Err(Refusal::BadSignature)
        );
        let other = public_key_b64(&[8; 32]);
        assert_eq!(
            check(&bytes, &sig, &other, STABLE, None),
            Err(Refusal::BadSignature)
        );
        assert_eq!(
            check(&bytes, "AAAA", &key, STABLE, None),
            Err(Refusal::BadSignature)
        );
        assert_eq!(
            check(&bytes, &sig, "garbage", STABLE, None),
            Err(Refusal::BadKey)
        );
    }

    #[test]
    fn other_schemas_and_channels_are_refused() {
        let v2 = br#"{"schema":2,"tools":[]}"#;
        assert_eq!(accept(v2, None), Err(Refusal::Schema(2)));
        let test = doc(Some("test"), Some(9), "a");
        assert!(matches!(accept(&test, None), Err(Refusal::Channel { .. })));
        let sig = sign_bytes(&SEED, &test);
        assert!(check(&test, &sig, &public_key_b64(&SEED), "test", None).is_ok());
    }

    #[test]
    fn replayed_or_replaced_documents_are_refused() {
        let current =
            accept(&doc(None, Some(5), "a"), None).unwrap_or_else(|e| unreachable!("{e}"));
        let floor = current.floor;
        assert_eq!(
            accept(&doc(None, Some(4), "a"), Some(&floor)),
            Err(Refusal::Rollback { floor: 5, found: 4 })
        );
        assert_eq!(
            accept(&doc(None, Some(5), "b"), Some(&floor)),
            Err(Refusal::Rollback { floor: 5, found: 5 })
        );
        assert_eq!(
            accept(&doc(None, None, "legacy"), Some(&floor)),
            Err(Refusal::Rollback { floor: 5, found: 0 })
        );
        assert!(
            accept(&doc(None, Some(5), "a"), Some(&floor)).is_ok(),
            "same document"
        );
        assert!(
            accept(&doc(None, Some(6), "c"), Some(&floor)).is_ok(),
            "newer"
        );
    }

    #[test]
    fn unsequenced_documents_may_change_until_a_sequenced_one_is_accepted() {
        let legacy = accept(&doc(None, None, "a"), None).unwrap_or_else(|e| unreachable!("{e}"));
        assert!(accept(&doc(None, None, "b"), Some(&legacy.floor)).is_ok());
    }

    #[test]
    fn the_compiled_release_key_is_a_valid_public_key() {
        let key: Option<[u8; 32]> = b64_decode(RELEASE_PUBLIC_KEY).and_then(|v| v.try_into().ok());
        assert!(key.is_some_and(|k| VerifyingKey::from_bytes(&k).is_ok()));
    }
}
