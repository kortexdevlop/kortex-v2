//! `cargo xtask build <id…|all>`: the web site of tools (ADR 0035). The shared player is compiled
//! once with `dx` (staged in `.kortex/build/_player/public/`, its hashed files installed once at
//! `dist/run/assets/`); each tool then gets its own files in `dist/run/<id>/`
//! (`tool.toml`, locales, `engine/engine.wasm`, a one-line `engine/worker.js`) and the pages
//! composed from the player and those files (`index.html`, `sw.js`). Nothing is compiled per tool
//! except its engine.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{engine, fwd, tools};

/// Where the compiled web player is staged: its `index.html` is the template of every tool page,
/// its `assets/` are installed once at the root of the tool origin (`dist/run/assets/`).
const PLAYER: &str = ".kortex/build/_player/public";
/// The generic engine worker, shared by every tool origin page and by the engine host.
const KX_WORKER: &str = "web/run-host/kx-worker.js";

fn abs(p: &str) -> Result<PathBuf, String> {
    std::env::current_dir()
        .map(|d| d.join(p))
        .map_err(|e| e.to_string())
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in fs::read_dir(from)
        .map_err(|e| format!("{} : {e}", from.display()))?
        .filter_map(Result::ok)
    {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// `cargo xtask fingerprint <univers.nom | apps.nom>`: the receipt source hash (debugging aid).
pub fn print_fingerprint(args: &[String]) -> Result<(), String> {
    let id = args.first().ok_or("usage : cargo xtask fingerprint <id>")?;
    let root = abs(".")?;
    let dir = if let Some(name) = id.strip_prefix("apps.") {
        root.join("apps").join(name)
    } else {
        let tool = tools::discover()?
            .into_iter()
            .find(|t| t.contract.as_ref().is_ok_and(|c| c.tool.id == *id))
            .ok_or_else(|| format!("outil `{id}` introuvable"))?;
        abs(&tool.dir.to_string_lossy())?
    };
    println!("{}", source_fingerprint(&root, &dir)?);
    Ok(())
}

fn selected_ids(args: &[String]) -> Result<Vec<String>, String> {
    let ids: Vec<&String> = args.iter().filter(|arg| arg.as_str() != "--web").collect();
    if ids.is_empty() {
        return Err("usage : cargo xtask build <univers.nom… | all> [--web]".to_owned());
    }
    if ids.iter().any(|id| id.as_str() == "all") {
        if ids.len() != 1 {
            return Err("utilise `all` seul ou fournis une liste d’identifiants".to_owned());
        }
        return tools::discover()?
            .into_iter()
            .map(|t| {
                t.contract
                    .map(|c| c.tool.id)
                    .map_err(|e| format!("{} : {e}", t.dir.display()))
            })
            .collect();
    }
    Ok(ids.into_iter().cloned().collect())
}

pub fn run(args: &[String]) -> Result<(), String> {
    let ids = selected_ids(args)?;
    let root = abs(".")?;
    engine::build_engine(&ids)?;
    build_player_web(&root)?;
    for id in &ids {
        write_tool_files(&root, id)?;
    }
    compose(&root)?;
    // Receipts of the composed directories: the publication checks every file against them.
    for id in &ids {
        let tool_dir = root.join(tools::dir_of(id)?);
        write_receipt(&root, id, &tool_dir)?;
    }
    Ok(())
}

fn write_receipt(root: &Path, id: &str, tool_dir: &Path) -> Result<(), String> {
    let receipt = root
        .join(".kortex/build")
        .join(id)
        .join("build-receipt.json");
    fs::create_dir_all(receipt.parent().ok_or("chemin invalide")?).map_err(|e| e.to_string())?;
    fs::write(
        receipt,
        serde_json::json!({
            "source": engine_fingerprint(root, tool_dir)?,
            "output": tree_fingerprint(&root.join("dist/run").join(id))?,
        })
        .to_string(),
    )
    .map_err(|e| e.to_string())
}

/// `cargo xtask build-player-web`: compiles the web player once (receipt-cached) and composes the
/// pages of every tool directory already present in `dist/run`.
pub fn run_player(_args: &[String]) -> Result<(), String> {
    let root = abs(".")?;
    build_player_web(&root)?;
    compose(&root)
}

fn build_player_web(root: &Path) -> Result<(), String> {
    let dest = root.join(PLAYER);
    let receipt = root.join(".kortex/build/_player/build-receipt.json");
    let source = source_fingerprint(root, &root.join("crates/kortex-player"))?;
    if dest.is_dir()
        && let Ok(previous) = fs::read_to_string(&receipt)
    {
        let current =
            serde_json::json!({"source": source, "output": tree_fingerprint(&dest)?}).to_string();
        if previous == current {
            println!("✓ {PLAYER}/ à jour (compilation évitée)");
            return Ok(());
        }
    }
    let target = root.join(".kortex/target");
    // dx keeps stale hashed files from previous builds: start from a clean output dir.
    let _ = fs::remove_dir_all(target.join("dx").join("kortex-player"));
    println!("… compilation du lecteur web (dx build, premier build lent)");
    let status = Command::new("dx")
        .args([
            "build",
            "--release",
            "--platform",
            "web",
            "--package",
            "kortex-player",
            "--no-default-features",
            "--features",
            "web",
        ])
        .current_dir(root.join("crates/kortex-player"))
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .map_err(|e| format!("dx introuvable : {e}"))?;
    if !status.success() {
        return Err("dx build du lecteur web a échoué".to_owned());
    }
    let public = target.join("dx/kortex-player/release/web/public");
    let _ = fs::remove_dir_all(&dest);
    copy_dir(&public, &dest)?;
    if let Some(parent) = receipt.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        &receipt,
        serde_json::json!({"source": source, "output": tree_fingerprint(&dest)?}).to_string(),
    )
    .map_err(|e| e.to_string())?;
    let wasm = walk_size(&dest, "wasm");
    println!("✓ {PLAYER}/ publié (wasm : {} Ko)", wasm / 1024);
    Ok(())
}

/// The files that make a tool, without the pages composed from the player.
fn write_tool_files(root: &Path, id: &str) -> Result<(), String> {
    let tool = tools::discover()?
        .into_iter()
        .find(|t| t.contract.as_ref().is_ok_and(|c| c.tool.id == id))
        .ok_or_else(|| format!("outil `{id}` introuvable"))?;
    let tool_dir = root.join(&tool.dir);
    let dest = root.join("dist/run").join(id);
    let _ = fs::remove_dir_all(&dest);
    fs::create_dir_all(dest.join("locales")).map_err(|e| e.to_string())?;
    fs::create_dir_all(dest.join("engine")).map_err(|e| e.to_string())?;
    for name in ["tool.toml", "locales/fr.toml", "locales/en.toml"] {
        fs::copy(tool_dir.join(name), dest.join(name))
            .map_err(|e| format!("{id} : {name} : {e}"))?;
    }
    fs::copy(
        root.join("dist/engines").join(id).join("engine.wasm"),
        dest.join("engine/engine.wasm"),
    )
    .map_err(|e| format!("{id} : engine.wasm : {e}"))?;
    // The worker script lives inside the tool's scope so the offline cache controls it; it only
    // starts the generic worker for the tool's identifier.
    fs::write(
        dest.join("engine/worker.js"),
        format!("import {{ serve }} from '/engines/kx-worker.js';\nserve('{id}');\n"),
    )
    .map_err(|e| e.to_string())?;
    println!("✓ dist/run/{id}/");
    Ok(())
}

/// Installs the player's files at `dist/run/assets/`, writes `index.html` and `sw.js` of every
/// tool directory of `dist/run` from the current player, and the generic worker under
/// `dist/run/engines/`. Run again whenever the player changes: the
/// pages name its hashed files.
pub fn compose(root: &Path) -> Result<(), String> {
    let run = root.join("dist/run");
    let player = root.join(PLAYER);
    let index = fs::read(player.join("index.html"))
        .map_err(|e| format!("{PLAYER}/index.html : {e} (lance `cargo xtask build-player-web`)"))?;
    let worker = fs::read(root.join(KX_WORKER)).map_err(|e| format!("{KX_WORKER} : {e}"))?;
    fs::create_dir_all(run.join("engines")).map_err(|e| e.to_string())?;
    fs::write(run.join("engines/kx-worker.js"), &worker).map_err(|e| e.to_string())?;
    let mut shared = vec![("/engines/kx-worker.js".to_owned(), worker)];
    let mut player_files = Vec::new();
    collect_files(&player, &player, &mut player_files)?;
    // The hashed player files live once at the root of the tool origin, whatever the tool.
    let assets = run.join("assets");
    let _ = fs::remove_dir_all(&assets);
    copy_dir(&player.join("assets"), &assets)?;
    for relative in player_files {
        if relative == Path::new("index.html") {
            continue;
        }
        let bytes = fs::read(player.join(&relative)).map_err(|e| e.to_string())?;
        shared.push((format!("/{}", fwd(&relative)), bytes));
    }
    for entry in fs::read_dir(&run).map_err(|e| e.to_string())? {
        let dir = entry.map_err(|e| e.to_string())?.path();
        if !dir.join("tool.toml").is_file() {
            continue;
        }
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("nom de dossier invalide")?
            .to_owned();
        fs::write(dir.join("index.html"), &index).map_err(|e| e.to_string())?;
        write_offline_cache(&dir, &id, &shared)?;
    }
    Ok(())
}

fn collect_files(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect_files(&path, base, out)?;
        } else {
            out.push(
                path.strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_owned(),
            );
        }
    }
    out.sort();
    Ok(())
}

fn walk_size(dir: &Path, ext: &str) -> u64 {
    let mut total = 0;
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                total += walk_size(&p, ext);
            } else if p.extension().is_some_and(|x| x == ext) {
                total += p.metadata().map_or(0, |m| m.len());
            }
        }
    }
    total
}

/// Cache only the complete, immutable inventory of a tool page: its own files (everything but
/// the script itself), the shared player files and the generic worker.
fn write_offline_cache(dest: &Path, id: &str, shared: &[(String, Vec<u8>)]) -> Result<(), String> {
    use std::hash::{Hash, Hasher};
    let mut files = Vec::new();
    collect_files(dest, dest, &mut files)?;
    files.retain(|f| f != Path::new("sw.js"));
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let template = include_str!("../../crates/kortex-platform/src/offline-worker.js");
    template.hash(&mut hash);
    let mut urls = Vec::new();
    for relative in files {
        let name = fwd(&relative);
        name.hash(&mut hash);
        fs::read(dest.join(&relative))
            .map_err(|e| e.to_string())?
            .hash(&mut hash);
        urls.push(format!("/{id}/{name}"));
    }
    for (url, bytes) in shared {
        url.hash(&mut hash);
        bytes.hash(&mut hash);
        urls.push(url.clone());
    }
    // Directory navigation carries language/mode parameters but uses the same shell.
    urls.push(format!("/{id}/"));
    let prefix = format!("kortex-tool:{id}:");
    let cache = format!("{prefix}{:016x}", hash.finish());
    let script = template
        .replace(
            "__CACHE__",
            &serde_json::to_string(&cache).map_err(|e| e.to_string())?,
        )
        .replace(
            "__PREFIX__",
            &serde_json::to_string(&prefix).map_err(|e| e.to_string())?,
        )
        .replace(
            "__ASSETS__",
            &serde_json::to_string(&urls).map_err(|e| e.to_string())?,
        );
    fs::write(dest.join("sw.js"), script).map_err(|e| e.to_string())
}

// Local cache receipts include source and output content, so deleted/altered assets rebuild too.
pub(crate) fn tree_fingerprint(path: &Path) -> Result<String, String> {
    use std::hash::{Hash, Hasher};
    fn visit(path: &Path, base: &Path, hash: &mut impl Hasher) -> Result<(), String> {
        path.strip_prefix(base)
            .map_err(|e| e.to_string())?
            .hash(hash);
        if path.is_dir() {
            let mut entries = fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            entries.sort();
            for entry in entries {
                visit(&entry, base, hash)?;
            }
        } else {
            fs::read(path).map_err(|e| e.to_string())?.hash(hash);
        }
        Ok(())
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    visit(path, path, &mut hash)?;
    Ok(format!("{:016x}", hash.finish()))
}

/// `Cargo.lock` entries reachable from `roots` (workspace crates), canonicalised: a dependency
/// bump that no bundle input can see does not change the result.
pub(crate) fn lock_subset(lock: &str, roots: &[String]) -> Result<String, String> {
    use std::collections::{BTreeMap, BTreeSet};
    let table: toml::Table = lock.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let packages = table
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("Cargo.lock sans paquets")?;
    let mut by_name: BTreeMap<&str, Vec<&toml::Table>> = BTreeMap::new();
    for package in packages.iter().filter_map(toml::Value::as_table) {
        if let Some(name) = package.get("name").and_then(toml::Value::as_str) {
            by_name.entry(name).or_default().push(package);
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = roots.to_vec();
    let mut lines = Vec::new();
    while let Some(spec) = stack.pop() {
        // A dependency is `name`, `name version` or `name version (source)`.
        let mut parts = spec.split(' ');
        let (name, version) = (parts.next().unwrap_or_default(), parts.next());
        for package in by_name.get(name).into_iter().flatten() {
            let found = package.get("version").and_then(toml::Value::as_str);
            if version.is_some() && version != found {
                continue;
            }
            let key = format!("{name} {}", found.unwrap_or_default());
            if !seen.insert(key.clone()) {
                continue;
            }
            lines.push(format!("{package}"));
            stack.extend(
                package
                    .get("dependencies")
                    .and_then(toml::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|d| d.as_str().map(str::to_owned)),
            );
        }
    }
    lines.sort();
    Ok(lines.join("\n"))
}

/// Hash of everything a bundle build reads: the crate's own tree, the dependency closure of
/// workspace crates (plus the host wrapper libraries for a tool), the reachable lockfile
/// entries, the toolchain, the generated stylesheet and the xtask files that generate wrappers.
/// An unrelated crate, tool or xtask command never invalidates the receipt.
pub(crate) fn source_fingerprint(root: &Path, tool: &Path) -> Result<String, String> {
    fingerprint(root, tool, true)
}

/// Like [`source_fingerprint`] for the `engine.wasm` of a tool: only what the engine links (the
/// tool and its own dependencies), so an interface change never rebuilds engines.
pub(crate) fn engine_fingerprint(root: &Path, tool: &Path) -> Result<String, String> {
    fingerprint(root, tool, false)
}

fn fingerprint(root: &Path, tool: &Path, bundle: bool) -> Result<String, String> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    tree_fingerprint(tool)?.hash(&mut hash);
    let dir = tool
        .strip_prefix(root)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    let krates = crate::changed::workspace(root)?;
    let closure = if bundle {
        crate::changed::bundle_crates(&krates, &dir)
    } else {
        crate::changed::engine_crates(&krates, &dir)
    };
    if closure.is_empty() {
        return Err(format!(
            "{dir} : aucune crate du workspace à cet emplacement"
        ));
    }
    let mut files: Vec<&str> = vec!["Cargo.toml", "rust-toolchain.toml", ".cargo"];
    if bundle {
        files.push("design/dist/kortex.css");
        files.extend(
            crate::changed::GLOBAL_PATHS
                .iter()
                .copied()
                .filter(|p| p.starts_with("xtask/")),
        );
    } else {
        files.push("xtask/src/engine.rs");
    }
    for name in files {
        name.hash(&mut hash);
        let path = root.join(name);
        if path.exists() {
            tree_fingerprint(&path)?.hash(&mut hash);
        }
    }
    for krate in &closure {
        krate.name.hash(&mut hash);
        tree_fingerprint(&root.join(&krate.dir))?.hash(&mut hash);
    }
    let roots: Vec<String> = closure.iter().map(|k| k.name.clone()).collect();
    lock_subset(
        &fs::read_to_string(root.join("Cargo.lock")).map_err(|e| e.to_string())?,
        &roots,
    )?
    .hash(&mut hash);
    for name in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTUP_TOOLCHAIN"] {
        std::env::var_os(name).hash(&mut hash);
    }
    Ok(format!("{:016x}", hash.finish()))
}

#[cfg(test)]
mod fingerprint_tests {
    use super::lock_subset;

    const LOCK: &str = r#"
version = 4
[[package]]
name = "tool-a"
version = "0.1.0"
dependencies = ["kortex", "serde 1.0.1"]
[[package]]
name = "kortex"
version = "0.1.0"
dependencies = ["serde 1.0.1"]
[[package]]
name = "serde"
version = "1.0.1"
source = "registry+x"
checksum = "aa"
[[package]]
name = "serde"
version = "0.9.0"
source = "registry+x"
checksum = "bb"
[[package]]
name = "unrelated"
version = "0.1.0"
dependencies = ["ureq"]
[[package]]
name = "ureq"
version = "3.0.0"
source = "registry+x"
checksum = "cc"
"#;

    #[test]
    fn lock_subset_keeps_only_reachable_packages_with_the_right_version() {
        let subset = lock_subset(LOCK, &["tool-a".to_owned()]).unwrap_or_default();
        assert!(subset.contains("name = \"kortex\""));
        assert!(subset.contains("version = \"1.0.1\""));
        assert!(!subset.contains("0.9.0"), "unreachable serde version");
        assert!(!subset.contains("ureq") && !subset.contains("unrelated"));
    }

    #[test]
    fn lock_subset_changes_when_a_reachable_dependency_changes() {
        let bumped = LOCK.replace("checksum = \"aa\"", "checksum = \"zz\"");
        let roots = ["tool-a".to_owned()];
        assert_ne!(lock_subset(LOCK, &roots), lock_subset(&bumped, &roots));
        let unrelated = LOCK.replace("checksum = \"cc\"", "checksum = \"zz\"");
        assert_eq!(lock_subset(LOCK, &roots), lock_subset(&unrelated, &roots));
    }
}
