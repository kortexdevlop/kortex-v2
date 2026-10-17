//! `cargo xtask build <tool-id> [--web]`: builds the standalone web bundle of a tool.
//! A throwaway wrapper crate (Dioxus app = `AutoTool` + the tool's `run_map`) is generated in
//! `.kortex/build/<id>/`, compiled with `dx`, and published to `dist/run/<id>/`.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{fwd, tools};

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

pub fn run(args: &[String]) -> Result<(), String> {
    let ids: Vec<_> = args.iter().filter(|arg| arg.as_str() != "--web").collect();
    if ids.is_empty() {
        return Err("usage : cargo xtask build <univers.nom… | all> [--web]".to_owned());
    }
    if ids.iter().any(|id| id.as_str() == "all") {
        if ids.len() != 1 {
            return Err("utilise `all` seul ou fournis une liste d’identifiants".to_owned());
        }
        for t in tools::discover()? {
            let c = t
                .contract
                .map_err(|e| format!("{} : {e}", t.dir.display()))?;
            build_one(&c.tool.id)?;
        }
    } else {
        for id in ids {
            build_one(id)?;
        }
    }
    Ok(())
}

fn build_one(id: &str) -> Result<(), String> {
    let tool = tools::discover()?
        .into_iter()
        .find(|t| t.contract.as_ref().is_ok_and(|c| c.tool.id == id))
        .ok_or_else(|| format!("outil `{id}` introuvable"))?;
    let (universe, name) = id.split_once('.').ok_or("id invalide")?;
    let crate_name = format!("tool-{universe}-{name}");
    let lib_name = crate_name.replace('-', "_");
    let tool_dir = abs(&tool.dir.to_string_lossy())?;
    let root = abs(".")?;
    let work = root.join(".kortex/build").join(id);
    let dest = root.join("dist/run").join(id);
    let receipt = work.join("build-receipt.json");
    let source = source_fingerprint(&root, &tool_dir)?;
    if dest.is_dir()
        && let Ok(previous) = fs::read_to_string(&receipt)
    {
        let current =
            serde_json::json!({"source": source, "output": tree_fingerprint(&dest)?}).to_string();
        if previous == current {
            println!("✓ dist/run/{id}/ à jour (compilation évitée)");
            return Ok(());
        }
    }
    fs::create_dir_all(work.join("src")).map_err(|e| e.to_string())?;
    fs::create_dir_all(work.join("assets")).map_err(|e| e.to_string())?;
    let contract = tool.contract.as_ref().map_err(|e| e.to_string())?;
    let worker = if contract.tool.web_worker {
        Some(crate::worker::build(&root, &tool_dir, id)?)
    } else {
        None
    };
    let runner_body = if let Some((_, folder, codes)) = &worker {
        let timeout = contract
            .limits
            .timeout_s
            .unwrap_or(120)
            .saturating_mul(1000);
        format!(
            r#"match kortex_platform::worker::run("/{id}/{folder}/worker.js", inputs, params, {timeout}).await {{
            Ok(outputs) => Ok(outputs),
            Err(error) => {{
                let code = match error.code.as_str() {{
                    {codes}
                    "worker_timeout" => "worker_timeout",
                    "worker_protocol" => "worker_protocol",
                    "cancelled" => "cancelled",
                    _ => "worker_failed",
                }};
                Err(kortex_runtime::ToolError {{ code, detail: error.detail }})
            }}
        }}"#
        )
    } else {
        "let mut cx = kortex_runtime::Cx::new(); tool::run_map(&inputs, &params, &mut cx).await"
            .to_owned()
    };
    let cancel_prop = if worker.is_some() {
        "on_cancel: move |_| kortex_platform::worker::cancel(),"
    } else {
        ""
    };

    fs::write(
        work.join("Cargo.toml"),
        format!(
            r#"[package]
name = "run-{universe}-{name}"
version = "0.0.0"
edition = "2024"

[dependencies]
dioxus = {{ version = "0.7.10", features = ["web"] }}
{crate_name} = {{ path = "{tool}" }}
kortex-ui = {{ path = "{root}/crates/kortex-ui" }}
kortex-runtime = {{ path = "{root}/crates/kortex-runtime" }}
kortex-platform = {{ path = "{root}/crates/kortex-platform", features = ["document"] }}
kortex-contract = {{ path = "{root}/crates/kortex-contract" }}
kortex-i18n = {{ path = "{root}/crates/kortex-i18n" }}

[workspace]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
"#,
            tool = fwd(&tool_dir),
            root = fwd(&root)
        ),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        work.join("Dioxus.toml"),
        format!("[application]\n\n[web.app]\ntitle = \"Kortex\"\nbase_path = \"{id}\"\n"),
    )
    .map_err(|e| e.to_string())?;
    fs::copy("design/dist/kortex.css", work.join("assets/kortex.css"))
        .map_err(|e| format!("kortex.css : {e} (lance `cargo xtask tokens`)"))?;
    if !work.join("Cargo.lock").exists() {
        let _ = fs::copy("Cargo.lock", work.join("Cargo.lock"));
    }
    let web_source = format!(
        r#"// GENERATED by `cargo xtask build` — do not edit.
use dioxus::prelude::*;
use kortex_runtime::ValueMap;
use kortex_ui::{{AutoTool, RunFuture, Runner}};
use {lib_name} as tool;

const CSS: Asset = asset!("/assets/kortex.css");

fn runner(inputs: ValueMap, params: ValueMap) -> RunFuture {{
    Box::pin(async move {{
        {runner_body}
    }})
}}

fn main() {{
    kortex_platform::offline::register("/{id}/sw.js");
    dioxus::launch(App);
}}

#[component]
fn App() -> Element {{
    // The bus script lives for the whole session; mode changes from the host arrive on its channel.
    let mut host_mode = use_signal(|| None::<String>);
    let mut host_lang = use_signal(|| None::<String>);
    use_future(move || async move {{
        let mut bus = kortex_platform::document::host_bus();
        while let Ok(mode) = bus.recv::<String>().await {{
            if let Some(lang) = mode.strip_prefix("lang:").filter(|l| ["fr","en"].contains(l)) {{ host_lang.set(Some(lang.to_owned())); }} else {{ host_mode.set(Some(mode)); }}
        }}
    }});
    let query = use_resource(|| async {{
        let mut ev = kortex_platform::document::launch_query();
        ev.recv::<String>().await.unwrap_or_else(|_| "fr||".to_owned())
    }});
    // Wait for the URL query: initial param values (variant presets) are computed once.
    let Some(query) = query.read().clone() else {{
        return rsx! {{}};
    }};
    let mut parts = query.split('|');
    let query_lang = parts.next().unwrap_or("fr").to_owned();
    let lang = host_lang().unwrap_or(query_lang);
    let variant = parts.next().filter(|v| !v.is_empty()).map(str::to_owned);
    let initial_mode = parts.next().filter(|m| !m.is_empty()).map(str::to_owned);
    let contract = kortex_contract::parse(tool::CONTRACT).expect("contract");
    let locale_src = if lang == "en" {{ include_str!("{tool_dir}/locales/en.toml") }} else {{ include_str!("{tool_dir}/locales/fr.toml") }};
    let locale = kortex_i18n::Catalog::from_toml(locale_src).unwrap_or_default();
    rsx! {{
        document::Stylesheet {{ href: CSS }}
        main {{ style: "padding: var(--k-space-4); max-width: 720px; margin: 0 auto;",
            AutoTool {{ contract, lang, locale, runner: Runner(runner), variant, host_mode: host_mode().or(initial_mode), save_file: None, {cancel_prop} }}
        }}
    }}
}}
"#,
        tool_dir = fwd(&tool_dir)
    );
    crate::platform_check::check_source(&web_source)?;
    fs::write(work.join("src/main.rs"), web_source).map_err(|e| e.to_string())?;

    let target = root.join(".kortex/target");
    // dx keeps stale hashed files from previous builds: start from a clean output dir.
    let _ = fs::remove_dir_all(target.join("dx").join(format!("run-{universe}-{name}")));
    println!("… compilation de {id} (dx build, premier build lent)");
    let status = Command::new("dx")
        .args(["build", "--release", "--platform", "web"])
        .current_dir(&work)
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .map_err(|e| format!("dx introuvable : {e}"))?;
    if !status.success() {
        return Err(format!("dx build a échoué pour {id}"));
    }
    let public = target
        .join("dx")
        .join(format!("run-{universe}-{name}"))
        .join("release/web/public");
    let _ = fs::remove_dir_all(&dest);
    copy_dir(&public, &dest)?;
    if let Some((source, folder, _)) = worker {
        copy_dir(&source, &dest.join(folder))?;
    }
    write_offline_cache(&dest, id)?;
    fs::write(
        &receipt,
        serde_json::json!({"source": source, "output": tree_fingerprint(&dest)?}).to_string(),
    )
    .map_err(|e| e.to_string())?;
    let wasm: u64 = walk_size(&dest, "wasm");
    println!("✓ dist/run/{id}/ publié (wasm : {} Ko)", wasm / 1024);
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

// Cache only the complete, immutable build inventory on the tool origin.
fn write_offline_cache(dest: &Path, id: &str) -> Result<(), String> {
    use std::hash::{Hash, Hasher};
    fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                collect(&path, files)?;
            } else {
                files.push(path);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    collect(dest, &mut files)?;
    files.sort();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let template = include_str!("../../crates/kortex-platform/src/offline-worker.js");
    template.hash(&mut hash);
    let mut urls = Vec::new();
    for path in files {
        let relative = path.strip_prefix(dest).map_err(|e| e.to_string())?;
        let relative = fwd(relative);
        relative.hash(&mut hash);
        fs::read(&path).map_err(|e| e.to_string())?.hash(&mut hash);
        urls.push(format!("/{id}/{relative}"));
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

pub(crate) fn source_fingerprint(root: &Path, tool: &Path) -> Result<String, String> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    tree_fingerprint(tool)?.hash(&mut hash);
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo",
        "design/dist/kortex.css",
        "xtask/src",
        "xtask/Cargo.toml",
    ] {
        name.hash(&mut hash);
        tree_fingerprint(&root.join(name))?.hash(&mut hash);
    }
    // All internal libraries except unrelated CLI/package management. New shared libraries
    // automatically participate without having to update this list.
    let mut crates = fs::read_dir(root.join("crates"))
        .map_err(|e| e.to_string())?
        .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    crates.sort();
    for path in crates {
        if path.is_dir()
            && !matches!(
                path.file_name().and_then(|n| n.to_str()),
                Some("kortex-cli" | "kortex-pkg")
            )
        {
            path.file_name().hash(&mut hash);
            tree_fingerprint(&path)?.hash(&mut hash);
        }
    }
    for name in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTUP_TOOLCHAIN"] {
        std::env::var_os(name).hash(&mut hash);
    }
    Ok(format!("{:016x}", hash.finish()))
}
