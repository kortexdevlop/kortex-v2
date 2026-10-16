//! `cargo xtask build <tool-id> [--web]`: builds the standalone web bundle of a tool.
//! A throwaway wrapper crate (Dioxus app = `AutoTool` + the tool's `run_map`) is generated in
//! `.kortex/build/<id>/`, compiled with `dx`, and published to `dist/run/<id>/`.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::tools;

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

// Host <-> tool bus (see docs/adr/0004-host-tool-bus.md). Only the embedding window and its exact
// origin are trusted; messages are plain JSON-like objects { type: "kortex:*", ... }.
const BUS_JS: &str = r#"
(() => {
  if (window.__kortexBus) return;
  window.__kortexBus = true;
  const host = (location.ancestorOrigins && location.ancestorOrigins[0]) || (document.referrer ? new URL(document.referrer).origin : '');
  if (!host || window.parent === window) return;
  const send = (msg) => window.parent.postMessage(msg, host);
  let pending = null;
  const deliver = () => {
    const input = document.querySelector('input[type=file]');
    if (!pending || !input) return false;
    const dt = new DataTransfer();
    for (const f of pending) { if (f instanceof File) dt.items.add(f); }
    pending = null;
    input.files = dt.files;
    input.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  };
  window.addEventListener('message', (e) => {
    if (e.source !== window.parent || e.origin !== host || !e.data || typeof e.data.type !== 'string') return;
    if (e.data.type === 'kortex:init') {
      if (e.data.theme === 'light' || e.data.theme === 'dark') document.documentElement.dataset.theme = e.data.theme;
      else delete document.documentElement.dataset.theme;
      if (['simple', 'medium', 'pro'].includes(e.data.mode) && typeof dioxus !== 'undefined') dioxus.send(e.data.mode);
    } else if (e.data.type === 'kortex:files' && Array.isArray(e.data.files)) {
      pending = e.data.files;
      let tries = 0;
      const t = setInterval(() => { if (deliver() || ++tries > 60) clearInterval(t); }, 100);
    }
  });
  send({ type: 'kortex:ready' });
})();
"#;

pub fn run(args: &[String]) -> Result<(), String> {
    let id = args
        .first()
        .ok_or("usage : cargo xtask build <univers.nom | all>")?;
    if id == "all" {
        for t in tools::discover()? {
            let c = t
                .contract
                .map_err(|e| format!("{} : {e}", t.dir.display()))?;
            build_one(&c.tool.id)?;
        }
        return Ok(());
    }
    build_one(id)
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
    fs::create_dir_all(work.join("src")).map_err(|e| e.to_string())?;
    fs::create_dir_all(work.join("assets")).map_err(|e| e.to_string())?;

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
kortex-contract = {{ path = "{root}/crates/kortex-contract" }}
kortex-i18n = {{ path = "{root}/crates/kortex-i18n" }}

[workspace]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
"#,
            tool = tool_dir.display(),
            root = root.display()
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
    fs::write(work.join("src/bus.js"), BUS_JS).map_err(|e| e.to_string())?;
    fs::write(
        work.join("src/main.rs"),
        format!(
            r#"// GENERATED by `cargo xtask build` — do not edit.
use dioxus::prelude::*;
use kortex_runtime::{{Cx, ValueMap}};
use kortex_ui::{{AutoTool, RunFuture, Runner}};
use {lib_name} as tool;

const CSS: Asset = asset!("/assets/kortex.css");
const BUS_JS: &str = include_str!("bus.js");

fn runner(inputs: ValueMap, params: ValueMap) -> RunFuture {{
    Box::pin(async move {{
        let mut cx = Cx::new();
        tool::run_map(&inputs, &params, &mut cx).await
    }})
}}

fn main() {{
    dioxus::launch(App);
}}

#[component]
fn App() -> Element {{
    // The bus script lives for the whole session; mode changes from the host arrive on its channel.
    let mut host_mode = use_signal(|| None::<String>);
    use_future(move || async move {{
        let mut bus = document::eval(BUS_JS);
        while let Ok(mode) = bus.recv::<String>().await {{
            host_mode.set(Some(mode));
        }}
    }});
    let query = use_resource(|| async {{
        let mut ev = document::eval("const q = new URLSearchParams(location.search); dioxus.send((q.get('lang') || 'fr') + '|' + (q.get('variant') || '') + '|' + (q.get('mode') || ''))");
        ev.recv::<String>().await.unwrap_or_else(|_| "fr||".to_owned())
    }});
    // Wait for the URL query: initial param values (variant presets) are computed once.
    let Some(query) = query.read().clone() else {{
        return rsx! {{}};
    }};
    let mut parts = query.split('|');
    let lang = parts.next().unwrap_or("fr").to_owned();
    let variant = parts.next().filter(|v| !v.is_empty()).map(str::to_owned);
    let initial_mode = parts.next().filter(|m| !m.is_empty()).map(str::to_owned);
    let contract = kortex_contract::parse(tool::CONTRACT).expect("contract");
    let locale_src = if lang == "en" {{ include_str!("{tool_dir}/locales/en.toml") }} else {{ include_str!("{tool_dir}/locales/fr.toml") }};
    let locale = kortex_i18n::Catalog::from_toml(locale_src).unwrap_or_default();
    rsx! {{
        document::Stylesheet {{ href: CSS }}
        main {{ style: "padding: var(--k-space-4); max-width: 720px; margin: 0 auto;",
            AutoTool {{ contract, lang, locale, runner: Runner(runner), variant, host_mode: host_mode().or(initial_mode), save_file: None }}
        }}
    }}
}}
"#,
            tool_dir = tool_dir.display()
        ),
    )
    .map_err(|e| e.to_string())?;

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
    let dest = root.join("dist/run").join(id);
    let _ = fs::remove_dir_all(&dest);
    copy_dir(&public, &dest)?;
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
