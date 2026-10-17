//! Contract-based application discovery and registry generation.
use kortex_contract::{AppContract, parse_app, validate_app};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct App {
    pub dir: PathBuf,
    pub contract: AppContract,
}

pub fn discover() -> Result<Vec<App>, String> {
    let root = Path::new("apps");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut dirs = fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    dirs.sort();
    let mut apps = Vec::new();
    for dir in dirs.into_iter().filter(|d| d.is_dir()) {
        let text = fs::read_to_string(dir.join("app.toml"))
            .map_err(|e| format!("{} : {e}", dir.display()))?;
        let contract = parse_app(&text).map_err(|e| format!("{} : {e}", dir.display()))?;
        let issues = validate_app(&contract);
        if !issues.is_empty() {
            return Err(format!(
                "{} : {}",
                dir.display(),
                issues
                    .iter()
                    .map(|e| e.0.as_str())
                    .collect::<Vec<_>>()
                    .join(" ; ")
            ));
        }
        if contract.app.id
            != format!(
                "apps.{}",
                dir.file_name().unwrap_or_default().to_string_lossy()
            )
        {
            return Err(format!(
                "{} : app.id ne correspond pas au dossier",
                dir.display()
            ));
        }
        apps.push(App { dir, contract });
    }
    Ok(apps)
}

pub fn registry() -> Result<String, String> {
    let apps = discover()?
        .into_iter()
        .map(|a| a.contract)
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&serde_json::json!({"schema":1,"apps":apps}))
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

pub(crate) fn fingerprint(id: &str) -> Result<String, String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let name = id.strip_prefix("apps.").ok_or("invalid app id")?;
    let base = crate::build::source_fingerprint(&root, &root.join("apps").join(name))?;
    let extra = ["brand", "registry/index.json", "crates/kortex-pkg"]
        .iter()
        .map(|p| crate::build::tree_fingerprint(&root.join(p)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{base}:{}", extra.join(":")))
}
pub(crate) fn receipt(id: &str, target: &str, source: &str, output: &Path) -> Result<(), String> {
    if fingerprint(id)? != source {
        return Err(format!(
            "{id}: sources changed during compilation; rebuild required"
        ));
    }
    let dir = Path::new(".kortex/app-receipts");
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fn files(
        base: &Path,
        path: &Path,
        result: &mut std::collections::BTreeMap<String, String>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                files(base, &path, result)?;
            } else {
                result.insert(
                    path.strip_prefix(base)
                        .map_err(|e| e.to_string())?
                        .to_string_lossy()
                        .into_owned(),
                    kortex_pkg::sha256_hex(&fs::read(&path).map_err(|e| e.to_string())?),
                );
            }
        }
        Ok(())
    }
    let mut manifest = std::collections::BTreeMap::new();
    files(output, output, &mut manifest)?;
    fs::write(dir.join(format!("{id}-{target}.json")), serde_json::json!({"source":source,"output":crate::build::tree_fingerprint(output)?,"target":target,"path":output,"files":manifest}).to_string()).map_err(|e|e.to_string())
}

/// Compile the same app package with the web renderer and prepare its static deployment.
pub fn build_web(id: &str) -> Result<(), String> {
    let app = discover()?
        .into_iter()
        .find(|a| a.contract.app.id == id)
        .ok_or("application inconnue")?;
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let package = format!(
        "kortex-{}",
        id.strip_prefix("apps.")
            .ok_or("identifiant d'application invalide")?
    );
    let fingerprint = fingerprint(id)?;
    let target = root.join(".kortex/target-apps");
    let status = std::process::Command::new("dx")
        .args([
            "build",
            "--release",
            "--platform",
            "web",
            "--package",
            &package,
            "--no-default-features",
            "--features",
            "web",
        ])
        .current_dir(root.join(&app.dir))
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("compilation web de {id} échouée"));
    }
    let source = target.join("dx").join(&package).join("release/web/public");
    let destination = root.join("dist/apps").join(id);
    fn copy(source: &Path, destination: &Path) -> Result<(), String> {
        fs::create_dir_all(destination).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let to = destination.join(entry.file_name());
            if path.is_dir() {
                copy(&path, &to)?;
            } else {
                fs::copy(path, to).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|e| e.to_string())?;
    }
    copy(&source, &destination)?;
    let html = fs::read_to_string(destination.join("index.html")).map_err(|e| e.to_string())?;
    fs::create_dir_all(destination.join("en")).map_err(|e| e.to_string())?;
    fs::write(destination.join("en/index.html"), html).map_err(|e| e.to_string())?;
    receipt(id, "web", &fingerprint, &destination)?;
    println!("✓ {id} web → {}", destination.display());
    Ok(())
}
