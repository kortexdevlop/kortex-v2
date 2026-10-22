//! Modular catalogue: a small signed discovery index and immutable, hash-bound contracts.
//! Hosts fetch only `descriptor(id).path`, enforcing its byte limit before buffering.
use crate::{Accepted, Floor, Refusal, check, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const FORMAT: u32 = 2;
pub const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_FRAGMENT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TOOLS: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub version: String,
    pub name: BTreeMap<String, String>,
    pub universe: String,
    pub platforms: Vec<String>,
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub schema: u32,
    pub kind: String,
    pub format: u32,
    pub channel: String,
    pub sequence: u64,
    pub tools: BTreeMap<String, Descriptor>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fragment {
    schema: u32,
    kind: String,
    format: u32,
    id: String,
    version: String,
    contract: Value,
}

/// Immutable snapshot: a failed refresh leaves this verified instance usable.
/// Keep its floor separately from the legacy catalogue floor.
#[derive(Debug)]
pub struct Catalog {
    index: Index,
    pub accepted: Accepted,
}

fn invalid(message: &str) -> Refusal {
    Refusal::Unreadable(message.to_owned())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.split('.').count() == 2
        && id.split('.').all(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
}

fn identity(contract: &Value) -> Result<(&str, &str), Refusal> {
    if contract["schema"].as_u64() != Some(1) {
        return Err(invalid("unsupported contract schema"));
    }
    let tool = &contract["tool"];
    let id = tool["id"]
        .as_str()
        .filter(|id| valid_id(id))
        .ok_or_else(|| invalid("invalid contract id"))?;
    let version = tool["version"]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 128)
        .ok_or_else(|| invalid("invalid contract version"))?;
    Ok((id, version))
}

impl Catalog {
    /// Verify the complete root before exposing discovery entries. No fragment is parsed here.
    pub fn open(
        bytes: &[u8],
        signature: &str,
        key: &str,
        channel: &str,
        floor: Option<&Floor>,
    ) -> Result<Self, Refusal> {
        if bytes.len() > MAX_INDEX_BYTES {
            return Err(invalid("catalogue index too large"));
        }
        let accepted = check(bytes, signature, key, channel, floor)?;
        let index: Index = serde_json::from_slice(bytes).map_err(|e| invalid(&e.to_string()))?;
        if index.kind != "catalog-index" || index.format != FORMAT || index.sequence == 0 {
            return Err(invalid("unsupported modular catalogue format"));
        }
        if index.tools.len() > MAX_TOOLS {
            return Err(invalid("too many catalogue entries"));
        }
        for (id, d) in &index.tools {
            if !valid_id(id)
                || d.version.is_empty()
                || d.version.len() > 128
                || d.bytes == 0
                || d.bytes > MAX_FRAGMENT_BYTES
                || d.sha256.len() != 64
                || !d
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || d.path != format!("contracts/{}.json", d.sha256)
            {
                return Err(invalid("invalid catalogue descriptor"));
            }
        }
        Ok(Self { index, accepted })
    }

    pub fn entries(&self) -> &BTreeMap<String, Descriptor> {
        &self.index.tools
    }
    pub fn descriptor(&self, id: &str) -> Option<&Descriptor> {
        self.index.tools.get(id)
    }

    /// Accept bytes only for a selected member of this snapshot; never trust a path from a caller.
    pub fn resolve(&self, id: &str, bytes: &[u8]) -> Result<Value, Refusal> {
        let d = self
            .descriptor(id)
            .ok_or_else(|| invalid("unknown catalogue id"))?;
        if bytes.len() != d.bytes || sha256_hex(bytes) != d.sha256 {
            return Err(invalid("contract size or digest mismatch"));
        }
        let fragment: Fragment =
            serde_json::from_slice(bytes).map_err(|e| invalid(&e.to_string()))?;
        if fragment.schema != 1
            || fragment.kind != "catalog-contract"
            || fragment.format != FORMAT
            || fragment.id != id
            || fragment.version != d.version
            || identity(&fragment.contract)? != (id, d.version.as_str())
        {
            return Err(invalid("contract identity or format mismatch"));
        }
        Ok(fragment.contract)
    }
}

/// Release tooling writes these beside (never instead of) catalog.json and its signature.
pub struct Publication {
    pub index: Vec<u8>,
    pub fragments: BTreeMap<String, Vec<u8>>,
}

pub fn build(contracts: &[Value], channel: &str, sequence: u64) -> Result<Publication, Refusal> {
    if sequence == 0 {
        return Err(invalid("modular catalogue requires a nonzero sequence"));
    }
    if contracts.len() > MAX_TOOLS {
        return Err(invalid("too many catalogue entries"));
    }
    let mut tools = BTreeMap::new();
    let mut fragments = BTreeMap::new();
    for contract in contracts {
        let (id, version) = identity(contract)?;
        let fragment = Fragment {
            schema: 1,
            kind: "catalog-contract".into(),
            format: FORMAT,
            id: id.into(),
            version: version.into(),
            contract: contract.clone(),
        };
        let bytes = serde_json::to_vec(&fragment).map_err(|e| invalid(&e.to_string()))?;
        if bytes.len() > MAX_FRAGMENT_BYTES {
            return Err(invalid("contract too large"));
        }
        let sha256 = sha256_hex(&bytes);
        let path = format!("contracts/{sha256}.json");
        let tool = &contract["tool"];
        let descriptor = Descriptor {
            version: version.into(),
            name: serde_json::from_value(tool["name"].clone())
                .map_err(|e| invalid(&e.to_string()))?,
            universe: tool["universe"]
                .as_str()
                .ok_or_else(|| invalid("missing universe"))?
                .into(),
            platforms: serde_json::from_value(tool["platforms"].clone())
                .map_err(|e| invalid(&e.to_string()))?,
            path: path.clone(),
            sha256,
            bytes: bytes.len(),
        };
        if tools.insert(id.to_owned(), descriptor).is_some() {
            return Err(invalid("duplicate contract id"));
        }
        fragments.insert(path, bytes);
    }
    let index = serde_json::to_vec(&Index {
        schema: 1,
        kind: "catalog-index".into(),
        format: FORMAT,
        channel: channel.into(),
        sequence,
        tools,
    })
    .map_err(|e| invalid(&e.to_string()))?;
    if index.len() > MAX_INDEX_BYTES {
        return Err(invalid("catalogue index too large"));
    }
    Ok(Publication { index, fragments })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{public_key_b64, sign_bytes};
    const SEED: [u8; 32] = [73; 32];

    fn contract(id: &str) -> Value {
        serde_json::json!({"schema":1,"tool":{"id":id,"version":"1.0.0","name":{"fr":"Test","en":"Test"},"universe":"test","platforms":["web","desktop"]},"operations":(0..10).map(|i| serde_json::json!({"id":format!("op{i}"),"inputs":[],"outputs":[{"id":"result","type":"text"}]})).collect::<Vec<_>>()})
    }
    fn open(publication: &Publication) -> Result<Catalog, Refusal> {
        Catalog::open(
            &publication.index,
            &sign_bytes(&SEED, &publication.index),
            &public_key_b64(&SEED),
            "stable",
            None,
        )
    }
    fn one() -> Publication {
        build(&[contract("test.one")], "stable", 7).unwrap()
    }
    fn rewrite_fragment(p: &mut Publication, mutate: impl FnOnce(&mut Value)) {
        let mut index: Index = serde_json::from_slice(&p.index).unwrap();
        let d = index.tools.get_mut("test.one").unwrap();
        let mut value: Value = serde_json::from_slice(&p.fragments[&d.path]).unwrap();
        mutate(&mut value);
        let bytes = serde_json::to_vec(&value).unwrap();
        d.sha256 = sha256_hex(&bytes);
        d.bytes = bytes.len();
        d.path = format!("contracts/{}.json", d.sha256);
        p.fragments.insert(d.path.clone(), bytes);
        p.index = serde_json::to_vec(&index).unwrap();
    }
    fn selected<'a>(p: &'a Publication, c: &Catalog) -> &'a [u8] {
        &p.fragments[&c.descriptor("test.one").unwrap().path]
    }

    #[test]
    fn index_opens_without_any_fragment_and_resolves_only_selected_contract() {
        let p = build(&[contract("test.one"), contract("test.two")], "stable", 7).unwrap();
        let c = open(&p).unwrap();
        assert_eq!(c.entries().len(), 2);
        assert_eq!(
            c.resolve("test.one", selected(&p, &c)).unwrap(),
            contract("test.one")
        );
        assert!(c.resolve("test.missing", b"{}").is_err());
    }
    #[test]
    fn refuses_corruption_truncation_and_cross_version_fragment() {
        let p = one();
        let c = open(&p).unwrap();
        let mut bytes = selected(&p, &c).to_vec();
        bytes[0] ^= 1;
        assert!(c.resolve("test.one", &bytes).is_err());
        assert!(c.resolve("test.one", &bytes[..bytes.len() - 1]).is_err());
        let mut v2 = contract("test.one");
        v2["tool"]["version"] = "2.0.0".into();
        let other = build(&[v2], "stable", 8).unwrap();
        assert!(
            c.resolve("test.one", other.fragments.values().next().unwrap())
                .is_err()
        );
    }
    #[test]
    fn refuses_even_signed_identity_and_format_mismatches() {
        for (field, value) in [
            ("id", "test.other"),
            ("version", "2.0.0"),
            ("kind", "wrong"),
        ] {
            let mut p = one();
            rewrite_fragment(&mut p, |v| v[field] = value.into());
            let c = open(&p).unwrap();
            assert!(c.resolve("test.one", selected(&p, &c)).is_err());
        }
        let mut p = one();
        rewrite_fragment(&mut p, |v| {
            v["contract"]["tool"]["id"] = "test.other".into()
        });
        let c = open(&p).unwrap();
        assert!(c.resolve("test.one", selected(&p, &c)).is_err());
    }
    #[test]
    fn refuses_paths_outside_exact_content_address_and_oversized_descriptors() {
        for path in [
            "../secret",
            "/secret",
            "contracts/../secret",
            "contracts/%2e%2e/secret",
            "https://evil.test/a",
            "contracts\\secret",
        ] {
            let mut p = one();
            let mut index: Index = serde_json::from_slice(&p.index).unwrap();
            index.tools.get_mut("test.one").unwrap().path = path.into();
            p.index = serde_json::to_vec(&index).unwrap();
            assert!(open(&p).is_err());
        }
        let mut p = one();
        let mut index: Index = serde_json::from_slice(&p.index).unwrap();
        index.tools.get_mut("test.one").unwrap().bytes = MAX_FRAGMENT_BYTES + 1;
        p.index = serde_json::to_vec(&index).unwrap();
        assert!(open(&p).is_err());
    }
    #[test]
    fn preserves_cached_snapshot_on_unsupported_update_and_refuses_rollback() {
        let p = one();
        let cached = open(&p).unwrap();
        let mut index: Index = serde_json::from_slice(&p.index).unwrap();
        index.format = 999;
        let newer = serde_json::to_vec(&index).unwrap();
        assert!(
            Catalog::open(
                &newer,
                &sign_bytes(&SEED, &newer),
                &public_key_b64(&SEED),
                "stable",
                Some(&cached.accepted.floor)
            )
            .is_err()
        );
        assert!(cached.resolve("test.one", selected(&p, &cached)).is_ok());
        let old = build(&[contract("test.one")], "stable", 6).unwrap();
        assert!(
            Catalog::open(
                &old.index,
                &sign_bytes(&SEED, &old.index),
                &public_key_b64(&SEED),
                "stable",
                Some(&cached.accepted.floor)
            )
            .is_err()
        );
    }
    #[test]
    fn refuses_foreign_signature_channel_and_duplicate_ids() {
        let p = one();
        assert!(
            Catalog::open(
                &p.index,
                &sign_bytes(&[1; 32], &p.index),
                &public_key_b64(&SEED),
                "stable",
                None
            )
            .is_err()
        );
        assert!(
            Catalog::open(
                &p.index,
                &sign_bytes(&SEED, &p.index),
                &public_key_b64(&SEED),
                "test",
                None
            )
            .is_err()
        );
        assert!(build(&[contract("test.one"), contract("test.one")], "stable", 1).is_err());
        assert!(build(&[contract("test.one")], "stable", 0).is_err());
        let mut unsupported = contract("test.one");
        unsupported["schema"] = 99.into();
        assert!(build(&[unsupported], "stable", 1).is_err());
    }
    #[test]
    fn three_thousand_tools_and_thirty_thousand_operations_load_one_fragment() {
        let contracts: Vec<_> = (0..3000)
            .map(|i| contract(&format!("test.tool{i}")))
            .collect();
        let start = std::time::Instant::now();
        let p = build(&contracts, "stable", 1).unwrap();
        let c = open(&p).unwrap();
        assert_eq!(c.entries().len(), 3000);
        let d = c.descriptor("test.tool2999").unwrap();
        let resolved = c.resolve("test.tool2999", &p.fragments[&d.path]).unwrap();
        assert_eq!(resolved["operations"].as_array().unwrap().len(), 10);
        let full = serde_json::to_vec(&contracts).unwrap().len();
        eprintln!(
            "synthetic: index={}B, contracts={}B, selected={}B, build+verify+resolve={:?}",
            p.index.len(),
            full,
            d.bytes,
            start.elapsed()
        );
        assert!(p.index.len() < full / 2);
        assert!(p.index.len() < MAX_INDEX_BYTES);
    }
}
