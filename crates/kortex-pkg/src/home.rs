use kortex_platform::secrets::{NativeSecretStore, SecretStore};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{Kind, PkgError, Result};
use crate::version::Version;

/// Where Kortex keeps installed tools: `~/.kortex` (or `$KORTEX_HOME`).
#[derive(Debug, Clone)]
pub struct Home {
    root: PathBuf,
    pub(crate) secrets: Arc<dyn SecretStore>,
    /// Where launchers (`.desktop`, `.app`, Start Menu entries) go; defaults to the user's home.
    user_home: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledTool {
    pub version: String,
    /// Seconds since the Unix epoch.
    pub installed_at: u64,
    /// Binary path relative to the Kortex home.
    pub binary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub tools: BTreeMap<String, InstalledTool>,
}

/// Tool ids end up in file paths, so they are validated before any use.
pub(crate) fn check_id(id: &str) -> Result<()> {
    let ok = id.split_once('.').is_some_and(|(u, n)| {
        let part = |s: &str, dash: bool| {
            !s.is_empty()
                && s.len() <= 40
                && s.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || (dash && c == '-'))
        };
        part(u, false) && part(n, true)
    });
    if ok {
        Ok(())
    } else {
        Err(PkgError::new(
            Kind::Unsafe,
            format!("identifiant d'outil invalide : {id}"),
        ))
    }
}

pub(crate) fn check_version(v: &str) -> Result<()> {
    Version::parse(v)
        .map(|_| ())
        .ok_or_else(|| PkgError::new(Kind::Unsafe, format!("version invalide : {v}")))
}

impl Home {
    /// `$KORTEX_HOME`, else `~/.kortex`.
    pub fn from_env() -> Result<Self> {
        let user_home = kortex_platform::native::env::var_os("HOME")
            .or_else(|| kortex_platform::native::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| PkgError::new(Kind::Io, "dossier personnel introuvable (HOME)"))?;
        let root = kortex_platform::native::env::var_os("KORTEX_HOME")
            .map_or_else(|| user_home.join(".kortex"), PathBuf::from);
        Ok(Self::at(root, user_home))
    }

    /// Explicit locations (tests, portable installs).
    #[must_use]
    pub fn at(root: impl Into<PathBuf>, user_home: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            secrets: Arc::new(NativeSecretStore),
            user_home: user_home.into(),
        }
    }

    /// Explicit backend injection for isolated tests and controlled hosts.
    #[must_use]
    pub fn with_secret_store(mut self, secrets: Arc<dyn SecretStore>) -> Self {
        self.secrets = secrets;
        self
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn user_home(&self) -> &Path {
        &self.user_home
    }

    #[must_use]
    pub fn tools_dir(&self) -> PathBuf {
        self.root.join("tools")
    }

    #[must_use]
    pub fn registry_path(&self) -> PathBuf {
        self.root.join("registry.json")
    }

    /// Absolute binary path for an installed tool entry.
    #[must_use]
    pub fn binary_path(&self, t: &InstalledTool) -> PathBuf {
        self.root.join(&t.binary)
    }

    pub fn load_registry(&self) -> Result<Registry> {
        match kortex_platform::native::fs::read(self.registry_path()) {
            Ok(b) => serde_json::from_slice(&b)
                .map_err(|e| PkgError::new(Kind::Parse, format!("registry.json illisible : {e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registry {
                schema: 1,
                tools: BTreeMap::new(),
            }),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomic write (temp file + rename): a crash never leaves a half-written registry.
    pub fn save_registry(&self, r: &Registry) -> Result<()> {
        kortex_platform::native::fs::create_dir_all(&self.root)?;
        let tmp = self.root.join(format!(
            ".registry.{}.tmp",
            kortex_platform::native::process_id()
        ));
        kortex_platform::native::fs::write(
            &tmp,
            serde_json::to_vec_pretty(&Registry {
                schema: 1,
                tools: r.tools.clone(),
            })
            .map_err(|e| PkgError::new(Kind::Parse, e.to_string()))?,
        )?;
        kortex_platform::native::fs::rename(&tmp, self.registry_path())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_cannot_escape_the_tools_directory() {
        for ok in ["pdf.merge", "secure.hash-calculator", "qr.generator"] {
            assert!(check_id(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "../x.y",
            "a/b.c",
            "a.b/../c",
            "pdf.merge/..",
            "PDF.merge",
            "a.b.c",
            "a.",
            ".b",
            "a b.c",
            "a.b\0",
            "a\\b.c",
        ] {
            assert!(check_id(bad).is_err(), "{bad:?}");
        }
        assert!(check_version("1.2.3").is_ok());
        assert!(check_version("../1.2.3").is_err());
    }
}
