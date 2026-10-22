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

use crate::tools;

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

pub(crate) fn abs(p: &str) -> Result<PathBuf, String> {
    std::env::current_dir()
        .map(|d| d.join(p))
        .map_err(|e| e.to_string())
}

fn resolve_desktop_target_dir(root: &Path, configured: Option<PathBuf>) -> PathBuf {
    match configured.filter(|path| !path.as_os_str().is_empty()) {
        Some(path) if path.is_absolute() => path,
        Some(path) => root.join(path),
        None => root.join(".kortex/target-desktop"),
    }
}

pub(crate) fn desktop_target_dir(root: &Path) -> PathBuf {
    resolve_desktop_target_dir(
        root,
        std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
    )
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
/// the public half to `crates/kortex-catalog/release.pub` (committed, embedded in every client).
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
    fs::write("crates/kortex-catalog/release.pub", &public).map_err(|e| e.to_string())?;
    println!(
        "✓ clé privée écrite dans {out} (NE JAMAIS la committer ; secret CI : KORTEX_SIGNING_KEY)"
    );
    println!("✓ clé publique : {public}  (crates/kortex-catalog/release.pub, à committer)");
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

/// `cargo xtask build-desktop <id…|all> [--target T] [--players-dir D]`: the desktop binary of each tool is the
/// shared player (compiled once per target) with the tool's payload written into its reserved
/// region (ADR 0035). Nothing is compiled per tool except its engine, when its sources changed.
pub fn build_desktop(args: &[String]) -> Result<(), String> {
    let which: Vec<&String> = args.iter().take_while(|a| !a.starts_with("--")).collect();
    if which.is_empty() {
        return Err("usage : cargo xtask build-desktop <outil…|all> [--target T]".to_owned());
    }
    let target = host_target(args)?;
    let mut ids = Vec::new();
    for arg in which {
        ids.extend(tool_ids(arg)?);
    }
    let players = flag(args, "--players-dir").map(PathBuf::from);
    crate::player::build_desktop(&ids, &target, players.as_deref())
}

/// `cargo xtask build-player [--target T]`: compiles the desktop player only (no tool) and leaves
/// it in `dist/players/<target>/`, ready to be published as a reusable file.
pub fn build_player(args: &[String]) -> Result<(), String> {
    let target = host_target(args)?;
    let root = abs(".")?;
    let out = crate::player::build_player(&root, &target)?;
    println!("✓ {}", out.display());
    Ok(())
}

/// `cargo xtask player-fingerprint`: hash of everything the desktop player is built from. A
/// published player is reusable while it equals the local value.
pub fn player_fingerprint(_args: &[String]) -> Result<(), String> {
    let root = abs(".")?;
    println!(
        "{}",
        crate::build::source_fingerprint(&root, &root.join("crates/kortex-player"))?
    );
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
    let build_root = desktop_target_dir(&abs(".")?);
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release", "-p", &meta.krate])
        .env("CARGO_TARGET_DIR", &build_root);
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

pub(crate) fn cli_version() -> Result<String, String> {
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
    let build_root = desktop_target_dir(&abs(".")?);
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release", "-p", "kortex-cli"])
        .env("CARGO_TARGET_DIR", &build_root);
    if Some(target.as_str()) != current_target() {
        cmd.args(["--target", &target]);
    }
    if !cmd.status().map_err(|e| e.to_string())?.success() {
        return Err("la compilation de la CLI a échoué".into());
    }
    let built = if Some(target.as_str()) == current_target() {
        build_root.join("release")
    } else {
        build_root.join(&target).join("release")
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

/// `cargo xtask release-index [--base-url URL] [--key-file F] [--channel stable|test]
/// [--previous index.json] [--sequence N] [--source-commit SHA]`: builds and signs
/// `dist/release/index.json` and the contract catalogue `dist/release/catalog.json` (ADR 0034).
/// With `--previous` (the index currently published, already verified by the caller) the packages
/// of this run replace their entries and every other entry is kept with its absolute URLs, so a
/// release can carry only what changed; the sequence is the previous one plus one.
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
    let files = package_files(Path::new("dist/release"))?;
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
        if m.target != "web" && !TARGETS.contains(&m.target.as_str()) {
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
    let channel = flag(args, "--channel").unwrap_or_else(|| "stable".into());
    if !["stable", "test"].contains(&channel.as_str()) {
        return Err(format!("canal inconnu : {channel} (stable ou test)"));
    }
    let source_commit = flag(args, "--source-commit");
    let (cli, tool_map, sequence) = merge_previous(args, &channel, cli, tool_map)?;
    let sequence = match flag(args, "--sequence") {
        Some(s) => s
            .parse::<u64>()
            .map_err(|_| "--sequence : entier attendu")?,
        None => sequence,
    };
    let index = Index {
        schema: 1,
        channel: channel.clone(),
        sequence,
        source_commit: source_commit.clone(),
        generated: iso_date(secs),
        cli,
        tools: tool_map,
    };
    let bytes = serde_json::to_vec_pretty(&index).map_err(|e| e.to_string())?;
    let sig = sign_bytes(&seed, &bytes);
    // Never publish an index that this repository's embedded public key would reject.
    let public =
        fs::read_to_string("crates/kortex-catalog/release.pub").map_err(|e| e.to_string())?;
    parse_verified(&bytes, &sig, public.trim()).map_err(|e| format!("la clé de signature ne correspond pas à crates/kortex-catalog/release.pub ({e}) : lance `cargo xtask keygen`"))?;
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
    let catalog = catalog_document(
        &channel,
        sequence,
        source_commit.as_deref(),
        index.generated.clone(),
    )?;
    let catalog_sig = sign_bytes(&seed, catalog.as_bytes());
    parse_verified_catalog(&catalog, &catalog_sig, public.trim())?;
    fs::write("dist/release/catalog.json", &catalog).map_err(|e| e.to_string())?;
    fs::write("dist/release/catalog.json.sig", &catalog_sig).map_err(|e| e.to_string())?;
    crate::catalog::publish(&catalog, &seed, Path::new("dist/release"), public.trim())?;
    println!(
        "✓ dist/release/index.json + catalog.json + .sig ({} outil(s), CLI : {}, canal {channel}, séquence {sequence})",
        index.tools.len(),
        index.cli.as_ref().map_or("non", |_| "oui")
    );
    Ok(())
}

/// Collect package archives while excluding catalogue-only payloads left by an earlier attempt.
fn package_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<_> = fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.to_string_lossy().ends_with(".tar.gz")
                && path.file_name().and_then(|name| name.to_str())
                    != Some("catalog-contracts-v2.tar.gz")
        })
        .collect();
    files.sort();
    Ok(files)
}

/// CLI release, tool releases and sequence of a merged index.
type Merged = (Option<CliRelease>, BTreeMap<String, ToolRelease>, u64);

/// Entries of the index published before this run, kept unless this run rebuilt them. Returns the
/// merged CLI and tools plus the next sequence (previous + 1, or 1 without a previous index).
fn merge_previous(
    args: &[String],
    channel: &str,
    cli: Option<CliRelease>,
    mut tools: BTreeMap<String, ToolRelease>,
) -> Result<Merged, String> {
    let Some(path) = flag(args, "--previous") else {
        return Ok((cli, tools, 1));
    };
    let text = fs::read_to_string(&path).map_err(|e| format!("{path} : {e}"))?;
    let previous: Index = serde_json::from_str(&text).map_err(|e| format!("{path} : {e}"))?;
    if previous.channel != channel {
        return Err(format!(
            "l'index précédent est du canal {} (attendu {channel})",
            previous.channel
        ));
    }
    for (id, release) in &previous.tools {
        if release
            .targets
            .values()
            .any(|a| !a.url.starts_with("https://"))
        {
            return Err(format!(
                "{id} : l'index précédent contient une URL relative"
            ));
        }
        match tools.get_mut(id) {
            None => {
                tools.insert(id.clone(), release.clone());
            }
            // Same version rebuilt for other targets only (e.g. the first web archives): keep the
            // targets published earlier. A new version replaces the old entry as a whole.
            Some(new) if new.version == release.version => {
                for (target, artifact) in &release.targets {
                    new.targets
                        .entry(target.clone())
                        .or_insert_with(|| artifact.clone());
                }
            }
            Some(_) => {}
        }
    }
    let cli = match (cli, previous.cli) {
        (Some(mut new), Some(old)) if new.version == old.version => {
            for (target, artifact) in old.targets {
                new.targets.entry(target).or_insert(artifact);
            }
            Some(new)
        }
        (Some(new), _) => Some(new),
        (None, old) => old,
    };
    Ok((cli, tools, previous.sequence + 1))
}

/// The signed contract catalogue: every contract of `registry/contracts.json`.
fn catalog_document(
    channel: &str,
    sequence: u64,
    source_commit: Option<&str>,
    generated: String,
) -> Result<String, String> {
    let registry: serde_json::Value =
        serde_json::from_str(&crate::registry::build()?).map_err(|e| e.to_string())?;
    let mut document = serde_json::json!({
        "schema": 1,
        "channel": channel,
        "sequence": sequence,
        "generated": generated,
        "tools": registry["tools"],
    });
    if let Some(commit) = source_commit {
        document["source_commit"] = commit.into();
    }
    serde_json::to_string_pretty(&document).map_err(|e| e.to_string())
}

/// Never publish a catalogue that the key embedded in the clients would reject.
fn parse_verified_catalog(document: &str, sig: &str, public: &str) -> Result<(), String> {
    kortex_catalog::check(
        document.as_bytes(),
        sig,
        public,
        serde_json::from_str::<serde_json::Value>(document).map_err(|e| e.to_string())?["channel"]
            .as_str()
            .unwrap_or("stable"),
        None,
    )
    .map(|_| ())
    .map_err(|e| format!("catalogue refusé par la clé publique des clients : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str, url: &str) -> ToolRelease {
        ToolRelease {
            name: BTreeMap::new(),
            universe: "text".into(),
            version: version.into(),
            targets: BTreeMap::from([(
                "x86_64-apple-darwin".to_owned(),
                Artifact {
                    url: url.into(),
                    sha256: "00".into(),
                    size: 1,
                },
            )]),
        }
    }

    #[test]
    fn an_incremental_index_keeps_unchanged_entries_with_absolute_urls_and_counts_up() {
        let previous = Index {
            schema: 1,
            channel: "stable".into(),
            sequence: 7,
            source_commit: None,
            generated: "2026-10-20".into(),
            cli: None,
            tools: BTreeMap::from([
                (
                    "text.a".to_owned(),
                    release("1.0.0", "https://example.test/rel-1/a.tar.gz"),
                ),
                (
                    "text.b".to_owned(),
                    release("1.0.0", "https://example.test/rel-1/b.tar.gz"),
                ),
            ]),
        };
        let path = std::env::temp_dir().join(format!("kortex-prev-{}.json", std::process::id()));
        fs::write(&path, serde_json::to_vec(&previous).expect("json")).expect("write");
        let args = vec!["--previous".to_owned(), path.to_string_lossy().into_owned()];
        let rebuilt = BTreeMap::from([(
            "text.a".to_owned(),
            release("1.0.1", "https://example.test/rel-2/a.tar.gz"),
        )]);
        let (_, merged, sequence) =
            merge_previous(&args, "stable", None, rebuilt.clone()).expect("merged");
        assert_eq!(sequence, 8);
        assert_eq!(
            merged["text.a"].version, "1.0.1",
            "rebuilt entries replace the old ones"
        );
        assert_eq!(merged["text.b"].version, "1.0.0", "other entries are kept");
        assert!(
            merged["text.b"].targets["x86_64-apple-darwin"]
                .url
                .starts_with("https://example.test/rel-1/")
        );
        assert!(
            merge_previous(&args, "test", None, rebuilt).is_err(),
            "another channel is refused"
        );
        // Same version, new target only (first web archives): earlier targets are kept.
        let web_only = BTreeMap::from([(
            "text.b".to_owned(),
            ToolRelease {
                targets: BTreeMap::from([(
                    "web".to_owned(),
                    Artifact {
                        url: "https://example.test/rel-2/web-b.tar.gz".into(),
                        sha256: "11".into(),
                        size: 2,
                    },
                )]),
                ..release("1.0.0", "")
            },
        )]);
        let (_, merged, _) = merge_previous(&args, "stable", None, web_only).expect("merged");
        assert_eq!(
            merged["text.b"].targets.len(),
            2,
            "desktop target kept next to the new web one"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn desktop_builders_share_a_resolved_cache_root() {
        let root = std::env::current_dir().expect("current directory");
        assert_eq!(
            super::resolve_desktop_target_dir(&root, None),
            root.join(".kortex/target-desktop")
        );
        assert_eq!(
            super::resolve_desktop_target_dir(&root, Some("custom-target".into())),
            root.join("custom-target")
        );
        assert_eq!(
            super::resolve_desktop_target_dir(&root, Some(root.clone())),
            root
        );
    }

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

    #[test]
    fn catalogue_archive_is_never_treated_as_a_package_on_release_retry() {
        let root =
            std::env::temp_dir().join(format!("kortex-release-retry-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let package = root.join("web-test.one-1.0.0.tar.gz");
        fs::write(&package, b"package").unwrap();
        assert_eq!(package_files(&root).unwrap(), vec![package.clone()]);
        fs::write(
            root.join("catalog-contracts-v2.tar.gz"),
            b"contract fragments, no package manifest",
        )
        .unwrap();
        assert_eq!(package_files(&root).unwrap(), vec![package]);
        fs::remove_dir_all(root).unwrap();
    }
}
