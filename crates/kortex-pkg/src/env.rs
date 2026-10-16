use crate::error::{Kind, PkgError, Result};
use crate::{Home, Index, current_target, fetch, parse_verified};

pub const DEFAULT_INDEX: &str = "https://kortex.site/registry/index.json";

/// Everything the package operations depend on, injectable so tests can use a local index and key.
#[derive(Debug, Clone)]
pub struct PkgEnv {
    pub home: Home,
    pub index_src: String,
    /// Base64 Ed25519 public key that must have signed the index.
    pub public_key: String,
    pub target: Option<String>,
    /// Base URL of the Kortex API (`$KORTEX_API`).
    pub api: String,
}

impl PkgEnv {
    /// Process environment: `$KORTEX_HOME`, `$KORTEX_INDEX`, the embedded release key, this platform.
    pub fn from_process() -> Result<Self> {
        Ok(Self {
            home: Home::from_env()?,
            index_src: std::env::var("KORTEX_INDEX").unwrap_or_else(|_| DEFAULT_INDEX.to_owned()),
            public_key: crate::RELEASE_PUBLIC_KEY.trim().to_owned(),
            target: current_target().map(str::to_owned),
            api: std::env::var("KORTEX_API").unwrap_or_else(|_| crate::DEFAULT_API.to_owned()),
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

    /// Fetches the index and its signature, and refuses anything not signed by our key.
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
        parse_verified(&bytes, &sig, &self.public_key)
    }
}
