use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;

use crate::error::{Kind, PkgError, Result};
use crate::home::{Home, InstalledTool, check_id, check_version};
use crate::index::{Artifact, Index, Manifest, ToolRelease, sha256_hex};
use crate::net::{fetch, resolve};
use crate::shortcuts;
use crate::version::is_newer;
use crate::{binary_name, current_target};

/// Largest package we accept (compressed), and largest single file after decompression.
const MAX_PACKAGE: u64 = 600 * 1024 * 1024;
const MAX_FILE: u64 = 800 * 1024 * 1024;
const MAX_MANIFEST: u64 = 64 * 1024;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn display_name(r: &ToolRelease, id: &str) -> String {
    r.name
        .get("en")
        .or_else(|| r.name.values().next())
        .cloned()
        .unwrap_or_else(|| id.to_owned())
}

/// Downloads an artifact and checks it against the signed size and SHA-256.
fn download(index_src: &str, a: &Artifact) -> Result<Vec<u8>> {
    let bytes = fetch(&resolve(index_src, &a.url), MAX_PACKAGE)?;
    if bytes.len() as u64 != a.size {
        return Err(PkgError::new(
            Kind::Integrity,
            format!(
                "taille inattendue : {} octets au lieu de {}",
                bytes.len(),
                a.size
            ),
        ));
    }
    if !sha256_hex(&bytes).eq_ignore_ascii_case(&a.sha256) {
        return Err(PkgError::new(
            Kind::Integrity,
            "empreinte SHA-256 différente de celle de l'index signé : paquet refusé",
        ));
    }
    Ok(bytes)
}

fn read_limited<R: Read>(r: R, max: u64) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    r.take(max + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > max {
        return Err(PkgError::new(
            Kind::Unsafe,
            "fichier du paquet trop volumineux",
        ));
    }
    Ok(buf)
}

/// Extracts a package through an allow-list: only `manifest.json` and the one expected binary,
/// both plain files. Anything else (links, directories, `../`, absolute paths) is refused.
fn extract(archive: &[u8], binary: &str) -> Result<(Manifest, Vec<u8>)> {
    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    let (mut manifest, mut bin) = (None, None);
    for entry in tar
        .entries()
        .map_err(|e| PkgError::new(Kind::Integrity, format!("archive illisible : {e}")))?
    {
        let entry = entry
            .map_err(|e| PkgError::new(Kind::Integrity, format!("archive corrompue : {e}")))?;
        let name = entry
            .path()
            .map_err(|e| PkgError::new(Kind::Unsafe, e.to_string()))?
            .to_string_lossy()
            .into_owned();
        let name = name.strip_prefix("./").unwrap_or(&name).to_owned();
        if !entry.header().entry_type().is_file() {
            return Err(PkgError::new(
                Kind::Unsafe,
                format!("entrée non autorisée dans le paquet : {name}"),
            ));
        }
        if name == "manifest.json" && manifest.is_none() {
            manifest = Some(read_limited(entry, MAX_MANIFEST)?);
        } else if name == binary && bin.is_none() {
            bin = Some(read_limited(entry, MAX_FILE)?);
        } else {
            return Err(PkgError::new(
                Kind::Unsafe,
                format!("fichier inattendu dans le paquet : {name}"),
            ));
        }
    }
    let (Some(m), Some(b)) = (manifest, bin) else {
        return Err(PkgError::new(
            Kind::Integrity,
            "paquet incomplet (manifest.json ou binaire manquant)",
        ));
    };
    let manifest: Manifest = serde_json::from_slice(&m)
        .map_err(|e| PkgError::new(Kind::Parse, format!("manifest illisible : {e}")))?;
    if manifest.binary != binary || !sha256_hex(&b).eq_ignore_ascii_case(&manifest.sha256_binary) {
        return Err(PkgError::new(
            Kind::Integrity,
            "le binaire ne correspond pas à son manifest",
        ));
    }
    Ok((manifest, b))
}

/// Installs (or reinstalls) `id` for `target` from the verified `index`.
/// `index_src` is where the index came from, to resolve relative artifact URLs.
pub fn install(
    home: &Home,
    index: &Index,
    index_src: &str,
    id: &str,
    target: &str,
    progress: &mut dyn FnMut(&str),
) -> Result<InstalledTool> {
    check_id(id)?;
    let release = index.tools.get(id).ok_or_else(|| {
        PkgError::new(
            Kind::NotFound,
            format!("outil introuvable dans l'index : {id}"),
        )
    })?;
    check_version(&release.version)?;
    let artifact = release.targets.get(target).ok_or_else(|| {
        PkgError::new(
            Kind::Unsupported,
            format!("{id} n'est pas disponible pour {target}"),
        )
    })?;
    progress("download");
    let archive = download(index_src, artifact)?;
    progress("verify");
    let expected = binary_name(id, target);
    let (manifest, binary) = extract(&archive, &expected)?;
    if manifest.id != id || manifest.version != release.version || manifest.target != target {
        return Err(PkgError::new(
            Kind::Integrity,
            "le manifest ne correspond pas à l'index signé (id, version ou plateforme)",
        ));
    }
    progress("install");
    let dir = home.tools_dir().join(id);
    let final_dir = dir.join(&release.version);
    let stage = dir.join(format!(".stage-{}-{}", std::process::id(), now()));
    std::fs::create_dir_all(&stage)?;
    let written = (|| -> Result<()> {
        let bin_path = stage.join(&expected);
        std::fs::write(&bin_path, &binary)?;
        shortcuts::set_exec(&bin_path)?;
        std::fs::write(
            stage.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest)
                .map_err(|e| PkgError::new(Kind::Parse, e.to_string()))?,
        )?;
        if final_dir.exists() {
            std::fs::remove_dir_all(&final_dir)?;
        }
        std::fs::rename(&stage, &final_dir)?;
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(e);
    }
    let rel = PathBuf::from("tools")
        .join(id)
        .join(&release.version)
        .join(&expected);
    let entry = InstalledTool {
        version: release.version.clone(),
        installed_at: now(),
        binary: rel.to_string_lossy().replace('\\', "/"),
    };
    let mut reg = home.load_registry()?;
    reg.tools.insert(id.to_owned(), entry.clone());
    home.save_registry(&reg)?;
    // Old versions are removed only once the new one is registered.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.filter_map(std::result::Result::ok) {
            if e.file_name().to_string_lossy() != release.version.as_str() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    shortcuts::create(
        home,
        id,
        &display_name(release, id),
        &home.binary_path(&entry),
    );
    progress("done");
    Ok(entry)
}

/// Updates `id` if the index has a newer version. Returns the new entry, or `None` if up to date.
pub fn update(
    home: &Home,
    index: &Index,
    index_src: &str,
    id: &str,
    target: &str,
    progress: &mut dyn FnMut(&str),
) -> Result<Option<InstalledTool>> {
    check_id(id)?;
    let reg = home.load_registry()?;
    let current = reg
        .tools
        .get(id)
        .ok_or_else(|| PkgError::new(Kind::NotFound, format!("{id} n'est pas installé")))?;
    let release = index
        .tools
        .get(id)
        .ok_or_else(|| PkgError::new(Kind::NotFound, format!("{id} n'est plus dans l'index")))?;
    if !is_newer(&release.version, &current.version) {
        return Ok(None);
    }
    install(home, index, index_src, id, target, progress).map(Some)
}

pub fn remove(home: &Home, id: &str, display: &str) -> Result<()> {
    check_id(id)?;
    let mut reg = home.load_registry()?;
    if reg.tools.remove(id).is_none() {
        return Err(PkgError::new(
            Kind::NotFound,
            format!("{id} n'est pas installé"),
        ));
    }
    home.save_registry(&reg)?;
    let dir = home.tools_dir().join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    shortcuts::remove(home, id, display);
    Ok(())
}

#[must_use]
pub fn installed_binary(home: &Home, id: &str) -> Option<PathBuf> {
    home.load_registry()
        .ok()?
        .tools
        .get(id)
        .map(|t| home.binary_path(t))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolStatus {
    pub id: String,
    pub name: std::collections::BTreeMap<String, String>,
    pub universe: String,
    pub available: String,
    pub installed: Option<String>,
    pub update_available: bool,
    /// False when the index has no build for this machine's platform.
    pub supported: bool,
}

#[must_use]
pub fn statuses(index: &Index, reg: &crate::Registry, target: Option<&str>) -> Vec<ToolStatus> {
    index
        .tools
        .iter()
        .map(|(id, r)| {
            let installed = reg.tools.get(id).map(|t| t.version.clone());
            ToolStatus {
                id: id.clone(),
                name: r.name.clone(),
                universe: r.universe.clone(),
                available: r.version.clone(),
                update_available: installed
                    .as_deref()
                    .is_some_and(|v| is_newer(&r.version, v)),
                installed,
                supported: target.is_some_and(|t| r.targets.contains_key(t)),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

fn check(name: &str, ok: bool, detail: impl Into<String>) -> Check {
    Check {
        name: name.to_owned(),
        ok,
        detail: detail.into(),
    }
}

/// Health report: home writable, registry readable, every installed binary present and untampered.
#[must_use]
pub fn doctor(home: &Home) -> Vec<Check> {
    let mut out = Vec::new();
    out.push(
        match std::fs::create_dir_all(home.root()).and_then(|()| {
            let probe = home.root().join(".write-test");
            std::fs::write(&probe, b"x")?;
            std::fs::remove_file(probe)
        }) {
            Ok(()) => check("dossier", true, home.root().display().to_string()),
            Err(e) => check("dossier", false, format!("{} : {e}", home.root().display())),
        },
    );
    out.push(match current_target() {
        Some(t) => check("plateforme", true, t),
        None => check(
            "plateforme",
            false,
            format!(
                "{}-{} n'est pas une plateforme livrée",
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        ),
    });
    let reg = match home.load_registry() {
        Ok(r) => {
            out.push(check(
                "registre",
                true,
                format!("{} outil(s) installé(s)", r.tools.len()),
            ));
            r
        }
        Err(e) => {
            out.push(check("registre", false, e.to_string()));
            return out;
        }
    };
    for (id, t) in &reg.tools {
        let path = home.binary_path(t);
        let manifest: Option<Manifest> = path
            .parent()
            .and_then(|d| std::fs::read(d.join("manifest.json")).ok())
            .and_then(|b| serde_json::from_slice(&b).ok());
        out.push(match (std::fs::read(&path), manifest) {
            (Ok(bytes), Some(m)) if sha256_hex(&bytes).eq_ignore_ascii_case(&m.sha256_binary) => {
                check(id, true, format!("v{} intact", t.version))
            }
            (Ok(_), Some(_)) => check(
                id,
                false,
                "binaire modifié depuis l'installation (empreinte différente) : réinstalle-le",
            ),
            (Ok(_), None) => check(id, false, "manifest.json manquant ou illisible"),
            (Err(_), _) => check(
                id,
                false,
                format!("binaire introuvable : {}", path.display()),
            ),
        });
    }
    out
}

/// Replaces the running CLI binary at `exe` with the version in the index, if newer.
/// The previous binary is kept as `<exe>.old` during the swap (works on Windows too).
pub fn self_update(
    index: &Index,
    index_src: &str,
    exe: &Path,
    target: &str,
    current_version: &str,
) -> Result<Option<String>> {
    let cli = index.cli.as_ref().ok_or_else(|| {
        PkgError::new(
            Kind::NotFound,
            "l'index ne contient pas de version de la CLI",
        )
    })?;
    if !is_newer(&cli.version, current_version) {
        return Ok(None);
    }
    check_version(&cli.version)?;
    let artifact = cli.targets.get(target).ok_or_else(|| {
        PkgError::new(
            Kind::Unsupported,
            format!("la CLI n'est pas disponible pour {target}"),
        )
    })?;
    let archive = download(index_src, artifact)?;
    let name = binary_name_cli(target);
    let (manifest, binary) = extract(&archive, &name)?;
    if manifest.id != "cli" || manifest.version != cli.version || manifest.target != target {
        return Err(PkgError::new(
            Kind::Integrity,
            "le manifest de la CLI ne correspond pas à l'index signé",
        ));
    }
    let new = exe.with_extension("new");
    let old = exe.with_extension("old");
    std::fs::write(&new, &binary)?;
    shortcuts::set_exec(&new)?;
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old)?;
    if let Err(e) = std::fs::rename(&new, exe) {
        let _ = std::fs::rename(&old, exe); // roll back: never leave the user without a CLI
        return Err(e.into());
    }
    let _ = std::fs::remove_file(&old);
    Ok(Some(cli.version.clone()))
}

/// File name of the CLI binary inside its package.
#[must_use]
pub fn binary_name_cli(target: &str) -> String {
    if target.contains("windows") {
        "kortex.exe".to_owned()
    } else {
        "kortex".to_owned()
    }
}
