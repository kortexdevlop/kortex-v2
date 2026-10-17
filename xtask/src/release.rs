//! Release tooling: `keygen`, `build-desktop`, `package`, `release-index`.
//!
//! Flow (see docs/guides/release.md): every OS builds and packages its own binaries; one signing job
//! gathers all packages, writes `index.json` and signs it with the private key kept in CI secrets.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::GzEncoder;
use kortex_pkg::{
    Artifact, CliRelease, Index, Manifest, TARGETS, ToolRelease, binary_name, binary_name_cli,
    current_target, parse_verified, public_key_b64, sha256_hex, sign_bytes,
};

use crate::{fwd, tools};

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn host_target(args: &[String]) -> Result<String, String> {
    let t = flag(args, "--target")
        .or_else(|| current_target().map(str::to_owned))
        .ok_or("plateforme non supportée : précise --target")?;
    if TARGETS.contains(&t.as_str()) {
        Ok(t)
    } else {
        Err(format!(
            "cible inconnue : {t} (attendu : {})",
            TARGETS.join(", ")
        ))
    }
}

fn abs(p: &str) -> Result<PathBuf, String> {
    std::env::current_dir()
        .map(|d| d.join(p))
        .map_err(|e| e.to_string())
}

fn b64_seed(s: &str) -> Result<[u8; 32], String> {
    // Reuse the pkg crate's decoder through a signature round-trip is overkill: decode inline.
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut acc, mut bits, mut out) = (0_u32, 0_u32, Vec::new());
    for c in s.trim().bytes().filter(|c| *c != b'=') {
        let v = T
            .iter()
            .position(|x| *x == c)
            .ok_or("clé de signature : base64 invalide")? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    out.try_into()
        .map_err(|_| "clé de signature : 32 octets attendus".to_owned())
}

fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |a, (i, b)| a | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                char::from(T[((n >> (18 - 6 * i)) & 63) as usize])
            } else {
                '='
            });
        }
    }
    out
}

/// `cargo xtask keygen --out <file>`: new Ed25519 release key. The seed goes to `<file>` (0600),
/// the public half to `crates/kortex-pkg/release.pub` (committed, embedded in every client).
pub fn keygen(args: &[String]) -> Result<(), String> {
    let out = flag(args, "--out").ok_or("usage : cargo xtask keygen --out <fichier-privé>")?;
    if Path::new(&out).exists() && !args.iter().any(|a| a == "--force") {
        return Err(format!(
            "{out} existe déjà (--force pour écraser : les clients existants ne reconnaîtront plus les nouvelles releases)"
        ));
    }
    let mut seed = [0_u8; 32];
    getrandom::fill(&mut seed).map_err(|e| e.to_string())?;
    fs::write(&out, format!("{}\n", b64(&seed))).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&out, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    let public = public_key_b64(&seed);
    fs::write("crates/kortex-pkg/release.pub", &public).map_err(|e| e.to_string())?;
    println!(
        "✓ clé privée écrite dans {out} (NE JAMAIS la committer ; secret CI : KORTEX_SIGNING_KEY)"
    );
    println!("✓ clé publique : {public}  (crates/kortex-pkg/release.pub, à committer)");
    Ok(())
}

fn tool_ids(arg: &str) -> Result<Vec<String>, String> {
    if arg != "all" {
        return Ok(vec![arg.to_owned()]);
    }
    tools::discover()?
        .into_iter()
        .map(|t| {
            t.contract
                .map(|c| c.tool.id)
                .map_err(|e| format!("{} : {e}", t.dir.display()))
        })
        .collect()
}

/// `cargo xtask build-desktop <id|all> [--target T]`: native Dioxus app per tool (no `dx` needed:
/// the CSS is embedded, so a plain `cargo build` produces a self-contained binary).
pub fn build_desktop(args: &[String]) -> Result<(), String> {
    let which = args
        .first()
        .ok_or("usage : cargo xtask build-desktop <outil|all> [--target T]")?;
    let target = host_target(args)?;
    for id in tool_ids(which)? {
        build_desktop_one(&id, &target)?;
    }
    Ok(())
}

fn build_desktop_one(id: &str, target: &str) -> Result<(), String> {
    let t = tools::discover()?
        .into_iter()
        .find(|t| t.contract.as_ref().is_ok_and(|c| c.tool.id == id))
        .ok_or_else(|| format!("outil `{id}` introuvable"))?;
    let c = t.contract.map_err(|e| e.clone())?;
    let (universe, name) = id.split_once('.').ok_or("id invalide")?;
    let crate_name = format!("tool-{universe}-{name}");
    let lib = crate_name.replace('-', "_");
    let root = abs(".")?;
    let tool_dir = abs(&t.dir.to_string_lossy())?;
    let work = root.join(".kortex/desktop").join(id);
    fs::create_dir_all(work.join("src")).map_err(|e| e.to_string())?;
    let pkg = format!("run-desktop-{universe}-{name}");
    fs::write(
        work.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{pkg}"
version = "{version}"
edition = "2024"

[dependencies]
dioxus = {{ version = "0.7.10", features = ["desktop"] }}
{crate_name} = {{ path = "{tool}" }}
kortex-ui = {{ path = "{root}/crates/kortex-ui" }}
kortex-platform = {{ path = "{root}/crates/kortex-platform" }}
kortex-runtime = {{ path = "{root}/crates/kortex-runtime" }}
kortex-contract = {{ path = "{root}/crates/kortex-contract" }}
kortex-i18n = {{ path = "{root}/crates/kortex-i18n" }}

[workspace]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = "symbols"
"#,
            version = c.tool.version,
            tool = fwd(&tool_dir),
            root = fwd(&root)
        ),
    )
    .map_err(|e| e.to_string())?;
    fs::copy("design/dist/kortex.css", work.join("src/kortex.css"))
        .map_err(|e| format!("kortex.css : {e} (lance `cargo xtask tokens`)"))?;
    let _ = fs::copy("Cargo.lock", work.join("Cargo.lock"));
    let title = c
        .tool
        .name
        .get("en")
        .cloned()
        .unwrap_or_else(|| id.to_owned());
    let desktop_source = format!(
        r##"// GENERATED by `cargo xtask build-desktop` — do not edit.
use dioxus::desktop::{{Config, LogicalSize, WindowBuilder}};
use dioxus::prelude::*;
use kortex_runtime::{{Cx, FileData, ValueMap}};
use kortex_ui::{{AutoTool, RunFuture, Runner, SaveFile}};
use {lib} as tool;

const CSS: &str = include_str!("kortex.css");

fn runner(inputs: ValueMap, params: ValueMap) -> RunFuture {{
    Box::pin(async move {{
        let mut cx = Cx::new();
        tool::run_map(&inputs, &params, &mut cx).await
    }})
}}

/// Saves a user-requested output through the native platform backend.
fn save(file: &FileData) -> Result<String, String> {{
    kortex_platform::native::save_download(&file.name, &file.bytes)
}}

fn main() {{
    let window = WindowBuilder::new().with_title("Kortex — {title}").with_inner_size(LogicalSize::new(620.0, 800.0));
    dioxus::LaunchBuilder::desktop().with_cfg(Config::new().with_window(window)).launch(App);
}}

#[component]
fn App() -> Element {{
    let lang = kortex_platform::native::env::var("KORTEX_LANG").ok().or_else(|| kortex_platform::native::env::var("LANG").ok()).filter(|l| l.starts_with("fr")).map_or("en", |_| "fr").to_owned();
    let contract = kortex_contract::parse(tool::CONTRACT).expect("contract");
    let locale_src = if lang == "fr" {{ include_str!("{tool_dir}/locales/fr.toml") }} else {{ include_str!("{tool_dir}/locales/en.toml") }};
    let locale = kortex_i18n::Catalog::from_toml(locale_src).unwrap_or_default();
    let mode = kortex_platform::native::env::var("KORTEX_MODE").ok();
    rsx! {{
        style {{ "{{CSS}}" }}
        main {{ style: "padding: var(--k-space-4); max-width: 720px; margin: 0 auto;",
            AutoTool {{ contract, lang, locale, runner: Runner(runner), variant: None::<String>, host_mode: mode, save_file: Some(SaveFile(save)) }}
        }}
    }}
}}
"##,
        tool_dir = fwd(&tool_dir)
    );
    crate::platform_check::check_source(&desktop_source)?;
    fs::write(work.join("src/main.rs"), desktop_source).map_err(|e| e.to_string())?;

    println!("… compilation desktop de {id} pour {target}");
    let target_dir = root.join(".kortex/target-desktop");
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release"])
        .current_dir(&work)
        .env("CARGO_TARGET_DIR", &target_dir);
    if Some(target) != current_target() {
        cmd.args(["--target", target]);
    }
    if !cmd.status().map_err(|e| e.to_string())?.success() {
        return Err(format!("la compilation desktop de {id} a échoué"));
    }
    let exe = if target.contains("windows") {
        format!("{pkg}.exe")
    } else {
        pkg
    };
    let built = if Some(target) == current_target() {
        target_dir.join("release").join(&exe)
    } else {
        target_dir.join(target).join("release").join(&exe)
    };
    let dest = root.join("dist/desktop").join(target).join(id);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    fs::copy(&built, dest.join(binary_name(id, target)))
        .map_err(|e| format!("{} : {e}", built.display()))?;
    println!("✓ dist/desktop/{target}/{id}/{}", binary_name(id, target));
    Ok(())
}

/// Metadata of an app (`apps/<name>/app.toml`), e.g. `apps.store`.
struct AppMeta {
    version: String,
    universe: String,
    name: BTreeMap<String, String>,
    krate: String,
}

fn app_meta(id: &str) -> Option<AppMeta> {
    let app = crate::apps::discover()
        .ok()?
        .into_iter()
        .find(|a| a.contract.app.id == id)?;
    let meta = app.contract.app;
    Some(AppMeta {
        version: meta.version,
        universe: meta.universe,
        name: meta.name,
        krate: format!("kortex-{}", id.strip_prefix("apps.")?),
    })
}

/// `cargo xtask build-app <apps.id> [--target T]`: native app from `apps/<name>` (e.g. the Store).
pub fn build_app(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "--web") {
        return crate::apps::build_web(args.first().ok_or("application manquante")?);
    }
    let id = args
        .first()
        .ok_or("usage : cargo xtask build-app <apps.nom> [--target T]")?;
    let meta = app_meta(id)
        .ok_or_else(|| format!("application `{id}` introuvable (apps/<nom>/app.toml)"))?;
    let fingerprint = crate::apps::fingerprint(id)?;
    let target = host_target(args)?;
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release", "-p", &meta.krate]);
    if Some(target.as_str()) != current_target() {
        cmd.args(["--target", &target]);
    }
    if !cmd.status().map_err(|e| e.to_string())?.success() {
        return Err(format!("la compilation de {id} a échoué"));
    }
    let exe = if target.contains("windows") {
        format!("{}.exe", meta.krate)
    } else {
        meta.krate.clone()
    };
    let build_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    let built = if Some(target.as_str()) == current_target() {
        build_root.join("release")
    } else {
        build_root.join(&target).join("release")
    }
    .join(&exe);
    let dest = Path::new("dist/desktop").join(&target).join(id);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    fs::copy(&built, dest.join(binary_name(id, &target)))
        .map_err(|e| format!("{} : {e}", built.display()))?;
    crate::apps::receipt(id, &target, &fingerprint, &dest)?;
    println!("✓ dist/desktop/{target}/{id}/{}", binary_name(id, &target));
    Ok(())
}

fn cli_version() -> Result<String, String> {
    let toml = fs::read_to_string("crates/kortex-cli/Cargo.toml").map_err(|e| e.to_string())?;
    toml.lines()
        .find_map(|l| {
            l.strip_prefix("version = \"")?
                .split('"')
                .next()
                .map(str::to_owned)
        })
        .ok_or_else(|| "version de la CLI introuvable".to_owned())
}

/// `cargo xtask build-cli [--target T]`: the `kortex` command-line binary.
pub fn build_cli(args: &[String]) -> Result<(), String> {
    let target = host_target(args)?;
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release", "-p", "kortex-cli"]);
    if Some(target.as_str()) != current_target() {
        cmd.args(["--target", &target]);
    }
    if !cmd.status().map_err(|e| e.to_string())?.success() {
        return Err("la compilation de la CLI a échoué".into());
    }
    let built = if Some(target.as_str()) == current_target() {
        PathBuf::from("target/release")
    } else {
        Path::new("target").join(&target).join("release")
    }
    .join(binary_name_cli(&target));
    let dest = Path::new("dist/desktop").join(&target).join("cli");
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    fs::copy(&built, dest.join(binary_name_cli(&target)))
        .map_err(|e| format!("{} : {e}", built.display()))?;
    println!("✓ dist/desktop/{target}/cli/{}", binary_name_cli(&target));
    Ok(())
}

/// `cargo xtask package <id|cli|all> [--target T]`: `dist/release/<id>-<version>-<target>.tar.gz`.
pub fn package(args: &[String]) -> Result<(), String> {
    let which = args
        .first()
        .ok_or("usage : cargo xtask package <outil|cli|all> [--target T]")?;
    let target = host_target(args)?;
    let mut ids = if which == "all" {
        tool_ids("all")?
    } else {
        vec![which.clone()]
    };
    if which == "all" {
        ids.extend(
            crate::apps::discover()?
                .into_iter()
                .map(|app| app.contract.app.id),
        );
        ids.push("cli".to_owned());
    }
    for id in ids {
        package_one(&id, &target)?;
    }
    Ok(())
}

fn package_one(id: &str, target: &str) -> Result<(), String> {
    let (version, bin_name) = if id == "cli" {
        (cli_version()?, binary_name_cli(target))
    } else {
        let v = match app_meta(id) {
            Some(a) => a.version,
            None => {
                tools::discover()?
                    .into_iter()
                    .find_map(|t| t.contract.ok().filter(|c| c.tool.id == id))
                    .ok_or_else(|| format!("outil `{id}` introuvable"))?
                    .tool
                    .version
            }
        };
        (v, binary_name(id, target))
    };
    let src = Path::new("dist/desktop")
        .join(target)
        .join(id)
        .join(&bin_name);
    let bin = fs::read(&src).map_err(|e| {
        format!(
            "{} : {e} (lance `build-desktop`/`build-cli` d'abord)",
            src.display()
        )
    })?;
    let manifest = serde_json::to_vec_pretty(&Manifest {
        id: id.to_owned(),
        version: version.clone(),
        target: target.to_owned(),
        binary: bin_name.clone(),
        sha256_binary: sha256_hex(&bin),
    })
    .map_err(|e| e.to_string())?;
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::best()));
    // Fixed metadata so identical inputs give identical archives.
    for (name, data, mode) in [
        ("manifest.json", manifest.as_slice(), 0o644),
        (bin_name.as_str(), bin.as_slice(), 0o755),
    ] {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(mode);
        h.set_mtime(0);
        h.set_uid(0);
        h.set_gid(0);
        h.set_entry_type(tar::EntryType::Regular);
        tar.append_data(&mut h, name, data)
            .map_err(|e| e.to_string())?;
    }
    let bytes = tar
        .into_inner()
        .map_err(|e| e.to_string())?
        .finish()
        .map_err(|e| e.to_string())?;
    fs::create_dir_all("dist/release").map_err(|e| e.to_string())?;
    let out = format!("dist/release/{id}-{version}-{target}.tar.gz");
    fs::write(&out, &bytes).map_err(|e| e.to_string())?;
    println!("✓ {out} ({} Ko)", bytes.len() / 1024);
    Ok(())
}

/// Days since epoch -> `YYYY-MM-DD` (civil-from-days, proleptic Gregorian).
fn iso_date(secs: u64) -> String {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn read_manifest(archive: &[u8]) -> Result<Manifest, String> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for e in tar.entries().map_err(|e| e.to_string())? {
        let mut e = e.map_err(|e| e.to_string())?;
        if e.path().map_err(|e| e.to_string())?.to_string_lossy() == "manifest.json" {
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut e, &mut buf).map_err(|e| e.to_string())?;
            return serde_json::from_slice(&buf).map_err(|e| e.to_string());
        }
    }
    Err("manifest.json absent du paquet".into())
}

/// `cargo xtask release-index [--base-url URL] [--key-file F]`: builds and signs `dist/release/index.json`.
/// The private key comes from `KORTEX_SIGNING_KEY` (CI secret) or `--key-file`.
pub fn release_index(args: &[String]) -> Result<(), String> {
    let base = flag(args, "--base-url").map(|b| b.trim_end_matches('/').to_owned());
    let seed_b64 = match (
        std::env::var("KORTEX_SIGNING_KEY"),
        flag(args, "--key-file"),
    ) {
        (Ok(k), _) if !k.trim().is_empty() => k,
        (_, Some(f)) => fs::read_to_string(&f).map_err(|e| format!("{f} : {e}"))?,
        _ => {
            return Err(
                "clé de signature absente : définis KORTEX_SIGNING_KEY ou passe --key-file".into(),
            );
        }
    };
    let seed = b64_seed(&seed_b64)?;
    let contracts: BTreeMap<String, _> = tools::discover()?
        .into_iter()
        .filter_map(|t| t.contract.ok())
        .map(|c| (c.tool.id.clone(), c))
        .collect();

    let mut tool_map: BTreeMap<String, ToolRelease> = BTreeMap::new();
    let mut cli: Option<CliRelease> = None;
    let mut files: Vec<_> = fs::read_dir("dist/release")
        .map_err(|e| format!("dist/release : {e}"))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".tar.gz"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err("aucun paquet dans dist/release (lance `package` d'abord)".into());
    }
    for path in files {
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let m = read_manifest(&bytes)?;
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("nom de fichier invalide")?
            .to_owned();
        let artifact = Artifact {
            url: base
                .as_ref()
                .map_or_else(|| file.clone(), |b| format!("{b}/{file}")),
            sha256: sha256_hex(&bytes),
            size: bytes.len() as u64,
        };
        if !TARGETS.contains(&m.target.as_str()) {
            return Err(format!("{file} : cible inconnue {}", m.target));
        }
        if m.id == "cli" {
            let r = cli.get_or_insert_with(|| CliRelease {
                version: m.version.clone(),
                targets: BTreeMap::new(),
            });
            if r.version != m.version {
                return Err(format!(
                    "{file} : version de la CLI {} différente de {} (une release = une version)",
                    m.version, r.version
                ));
            }
            r.targets.insert(m.target, artifact);
            continue;
        }
        let (name, universe) = match (contracts.get(&m.id), app_meta(&m.id)) {
            (Some(c), _) => (c.tool.name.clone(), c.tool.universe.clone()),
            (None, Some(a)) => (a.name, a.universe),
            (None, None) => {
                return Err(format!("{file} : {} absent du registre et des apps", m.id));
            }
        };
        let r = tool_map.entry(m.id.clone()).or_insert_with(|| ToolRelease {
            name,
            universe,
            version: m.version.clone(),
            targets: BTreeMap::new(),
        });
        if r.version != m.version {
            return Err(format!(
                "{file} : {} existe en versions {} et {} dans dist/release",
                m.id, r.version, m.version
            ));
        }
        r.targets.insert(m.target, artifact);
    }
    let secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        });
    let index = Index {
        schema: 1,
        generated: iso_date(secs),
        cli,
        tools: tool_map,
    };
    let bytes = serde_json::to_vec_pretty(&index).map_err(|e| e.to_string())?;
    let sig = sign_bytes(&seed, &bytes);
    // Never publish an index that this repository's embedded public key would reject.
    let public = fs::read_to_string("crates/kortex-pkg/release.pub").map_err(|e| e.to_string())?;
    parse_verified(&bytes, &sig, public.trim()).map_err(|e| format!("la clé de signature ne correspond pas à crates/kortex-pkg/release.pub ({e}) : lance `cargo xtask keygen`"))?;
    // Plain-text pointer for the shell/PowerShell installers (no JSON parser needed): "<target> <sha256> <url>".
    if let Some(c) = &index.cli {
        let lines: String = c
            .targets
            .iter()
            .map(|(t, a)| format!("{t} {} {}\n", a.sha256, a.url))
            .collect();
        fs::write("dist/release/cli-latest.txt", lines).map_err(|e| e.to_string())?;
    }
    fs::write("dist/release/index.json", &bytes).map_err(|e| e.to_string())?;
    fs::write("dist/release/index.json.sig", &sig).map_err(|e| e.to_string())?;
    println!(
        "✓ dist/release/index.json + .sig ({} outil(s), CLI : {})",
        index.tools.len(),
        index.cli.as_ref().map_or("non", |_| "oui")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(1_792_097_674), "2026-10-15");
        assert_eq!(iso_date(951_782_400), "2000-02-29");
    }

    #[test]
    fn seed_roundtrip() {
        let seed = [9_u8; 32];
        assert_eq!(b64_seed(&b64(&seed)), Ok(seed));
        assert!(b64_seed("short").is_err());
    }
}
