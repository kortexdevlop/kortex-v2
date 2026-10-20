use crate::error::{Kind, PkgError, Result};
use crate::{Home, Index, check_index, current_target, fetch, resolve};

pub const DEFAULT_INDEX: &str = "https://kortex.site/registry/index.json";

/// Everything the package operations depend on, injectable so tests can use a local index and key.
#[derive(Debug, Clone)]
pub struct PkgEnv {
    pub home: Home,
    pub index_src: String,
    /// Base64 Ed25519 public key that must have signed the index.
    pub public_key: String,
    pub target: Option<String>,
    /// Release channel this client follows (`$KORTEX_CHANNEL`, `stable` by default).
    pub channel: String,
    /// Base URL of the Kortex API (`$KORTEX_API`).
    pub api: String,
}

impl PkgEnv {
    /// Process environment: `$KORTEX_HOME`, `$KORTEX_INDEX`, the embedded release key, this platform.
    pub fn from_process() -> Result<Self> {
        Ok(Self {
            home: Home::from_env()?,
            index_src: kortex_platform::native::env::var("KORTEX_INDEX")
                .unwrap_or_else(|_| DEFAULT_INDEX.to_owned()),
            public_key: crate::RELEASE_PUBLIC_KEY.trim().to_owned(),
            target: current_target().map(str::to_owned),
            channel: kortex_platform::native::env::var("KORTEX_CHANNEL")
                .unwrap_or_else(|_| kortex_catalog::STABLE.to_owned()),
            api: kortex_platform::native::env::var("KORTEX_API")
                .unwrap_or_else(|_| crate::DEFAULT_API.to_owned()),
        })
    }

    pub fn target(&self) -> Result<&str> {
        self.target.as_deref().ok_or_else(|| {
            PkgError::new(
                Kind::Unsupported,
                format!(
                    "plateforme non supportée : {}-{}",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                ),
            )
        })
    }

    /// Fetches the index and its signature, and refuses anything not signed by our key, of
    /// another channel, or older than the last index accepted on this machine (anti-rollback).
    pub fn load_index(&self) -> Result<Index> {
        if self.public_key.starts_with("PLACEHOLDER") {
            return Err(PkgError::new(
                Kind::BadSignature,
                "clé de release non initialisée (lance `cargo xtask keygen`)",
            ));
        }
        let bytes = fetch(&self.index_src, 8 << 20)?;
        let sig = String::from_utf8(fetch(&format!("{}.sig", self.index_src), 4096)?)
            .map_err(|_| PkgError::new(Kind::Parse, "signature illisible"))?;
        let key = format!("index:{}", self.channel);
        let floor = self.home.floor(&key);
        let (index, accepted) = check_index(
            &bytes,
            &sig,
            &self.public_key,
            &self.channel,
            floor.as_ref(),
        )?;
        if floor.as_ref() != Some(&accepted) {
            self.home.save_floor(&key, accepted)?;
        }
        Ok(index)
    }

    /// Saves the contract of an installed tool (`tools/<id>/<version>/contract.json`), taken from
    /// the signed contract catalogue published next to the index (ADR 0034). Apps then describe
    /// the tool even if they were built before it existed. Best effort: `false` when the catalogue
    /// is unavailable, refused, or does not list this version.
    pub fn save_contract(&self, id: &str, version: &str) -> bool {
        self.try_save_contract(id, version).is_some()
    }

    /// Contracts of the signed catalogue published next to the index (verified: key, channel and
    /// anti-rollback floor), or `None` when it is unreachable, refused or absent.
    pub fn load_catalog(&self) -> Option<Vec<serde_json::Value>> {
        let url = resolve(&self.index_src, "catalog.json");
        let bytes = fetch(&url, 8 << 20).ok()?;
        let sig = String::from_utf8(fetch(&format!("{url}.sig"), 4096).ok()?).ok()?;
        let key = format!("catalog:{}", self.channel);
        let floor = self.home.floor(&key);
        let accepted = kortex_catalog::check(
            &bytes,
            &sig,
            &self.public_key,
            &self.channel,
            floor.as_ref(),
        )
        .ok()?;
        if floor.as_ref() != Some(&accepted.floor) {
            self.home.save_floor(&key, accepted.floor).ok()?;
        }
        let document: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        document["tools"].as_array().cloned()
    }

    fn try_save_contract(&self, id: &str, version: &str) -> Option<()> {
        crate::home::check_id(id).ok()?;
        let tools = self.load_catalog()?;
        let contract = tools.iter().find(|t| {
            t["tool"]["id"].as_str() == Some(id) && t["tool"]["version"].as_str() == Some(version)
        })?;
        let dir = self.home.tools_dir().join(id).join(version);
        kortex_platform::native::fs::is_dir(&dir).then_some(())?;
        let text = serde_json::to_vec_pretty(contract).ok()?;
        kortex_platform::native::fs::write(dir.join("contract.json"), text).ok()
    }
}
