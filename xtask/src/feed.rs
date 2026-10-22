//! Incremental publication (ADR 0034): what to rebuild since the published release, the web
//! archives that carry each tool/app bundle, and the full web tree rebuilt from the signed index.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use kortex_pkg::{Index, Manifest, is_newer, sha256_hex};
use serde_json::json;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// Versions of everything this checkout can publish: tools, apps, and the CLI.
fn current_versions() -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut all = std::collections::BTreeMap::new();
    for tool in crate::tools::discover()? {
        let c = tool
            .contract
            .map_err(|e| format!("{} : {e}", tool.dir.display()))?;
        all.insert(c.tool.id, c.tool.version);
    }
    for app in crate::apps::discover()? {
        all.insert(app.contract.app.id, app.contract.app.version);
    }
    all.insert("cli".to_owned(), crate::release::cli_version()?);
    Ok(all)
}

/// Ids that have a web bundle: every tool and every web app that is beta or stable.
fn web_ids() -> Result<(Vec<String>, Vec<String>), String> {
    let tools = crate::tools::discover()?
        .into_iter()
        .filter_map(|t| t.contract.ok().map(|c| c.tool.id))
        .collect();
    let apps = crate::apps::discover()?
        .into_iter()
        .filter(|a| {
            matches!(
                a.contract.app.status,
                kortex_contract::Status::Beta | kortex_contract::Status::Stable
            ) && a.contract.app.platforms.iter().any(|p| p == "web")
        })
        .map(|a| a.contract.app.id)
        .collect();
    Ok((tools, apps))
}

/// Pure decision: what to build given the published index and the files changed since its commit.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub reason: String,
    pub web_tools: BTreeSet<String>,
    pub web_apps: BTreeSet<String>,
    pub native_tools: BTreeSet<String>,
    pub native_apps: BTreeSet<String>,
    pub cli: bool,
    /// Ids changed without a version bump: the publication must stop.
    pub unbumped: Vec<String>,
}

impl Plan {
    fn nothing(&self) -> bool {
        self.web_tools.is_empty()
            && self.web_apps.is_empty()
            && self.native_tools.is_empty()
            && self.native_apps.is_empty()
            && !self.cli
    }
}

/// `files`: changed since the published commit (`None`: unknown, rebuild everything).
pub fn decide(
    previous: &Index,
    files: Option<&[String]>,
    krates: &[crate::changed::Krate],
    versions: &std::collections::BTreeMap<String, String>,
    web: &(Vec<String>, Vec<String>),
    force_all: bool,
) -> Plan {
    let mut plan = Plan::default();
    let has_web = |id: &str| {
        previous
            .tools
            .get(id)
            .is_some_and(|t| t.targets.contains_key("web"))
    };
    // Bootstrap only when a *published* entry has no web archive yet; an id the index has never
    // listed is simply new and is added below.
    let missing_web = web
        .0
        .iter()
        .chain(web.1.iter())
        .any(|id| previous.tools.contains_key(id) && !has_web(id));
    let is_new = |id: &String| !previous.tools.contains_key(id);
    let affected = files.map(|f| crate::changed::affected(f, krates));
    if missing_web || files.is_none() {
        // Bootstrap: the published release has no web archives (or no known commit). Everything
        // is built for the web; native packages are reused as published (same versions).
        plan.reason = if missing_web {
            "bootstrap: web archives missing"
        } else {
            "base commit unknown"
        }
        .into();
        plan.web_tools = web.0.iter().cloned().collect();
        plan.web_apps = web.1.iter().cloned().collect();
        // Ids the published index has never heard of (a new tool) have no desktop package yet.
        plan.native_tools = web
            .0
            .iter()
            .filter(|id| !previous.tools.contains_key(*id))
            .cloned()
            .collect();
        plan.native_apps = web
            .1
            .iter()
            .filter(|id| !previous.tools.contains_key(*id))
            .cloned()
            .collect();
        if force_all {
            plan.reason = "forced".into();
        } else {
            return plan;
        }
    }
    let (tools, apps, cli) = match (&affected, files) {
        _ if force_all => (
            web.0.iter().cloned().collect::<BTreeSet<_>>(),
            web.1.iter().cloned().collect::<BTreeSet<_>>(),
            true,
        ),
        (Some(a), Some(f)) => (
            a.tools.clone(),
            a.apps.clone(),
            a.all || crate::changed::touches(f, krates, "kortex-cli"),
        ),
        _ => (BTreeSet::new(), BTreeSet::new(), false),
    };
    let (mut tools, mut apps) = (tools, apps);
    tools.extend(web.0.iter().filter(|id| is_new(id)).cloned());
    apps.extend(web.1.iter().filter(|id| is_new(id)).cloned());
    if plan.reason.is_empty() {
        plan.reason = if tools.is_empty() && apps.is_empty() && !cli {
            "nothing to publish".into()
        } else {
            "changed since the published commit".into()
        };
    }
    for id in tools.iter().chain(apps.iter()) {
        let (Some(now), Some(published)) = (versions.get(id), previous.tools.get(id)) else {
            continue;
        };
        if !is_newer(now, &published.version) {
            plan.unbumped
                .push(format!("{id} ({now}, publiée : {})", published.version));
        }
    }
    if cli
        && let (Some(now), Some(published)) = (versions.get("cli"), &previous.cli)
        && !is_newer(now, &published.version)
    {
        plan.unbumped
            .push(format!("cli ({now}, publiée : {})", published.version));
    }
    plan.web_tools.extend(tools.iter().cloned());
    plan.web_apps.extend(apps.iter().cloned());
    plan.native_tools = tools;
    plan.native_apps = apps;
    plan.cli = cli;
    plan
}

/// What changed in `Cargo.lock` between two versions: the workspace crates whose entry changed (a
/// new tool crate, a bumped crate version) and whether any external dependency changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LockDelta {
    pub workspace: BTreeSet<String>,
    pub external: bool,
}

fn lock_packages(text: &str) -> std::collections::BTreeMap<String, (bool, String)> {
    text.split("[[package]]")
        .skip(1)
        .filter_map(|block| {
            let field = |key: &str| {
                block
                    .lines()
                    .find_map(|l| {
                        l.strip_prefix(key)?
                            .trim()
                            .strip_prefix('"')?
                            .split('"')
                            .next()
                    })
                    .map(str::to_owned)
            };
            let name = field("name =")?;
            let version = field("version =").unwrap_or_default();
            let external = block.lines().any(|l| l.starts_with("source ="));
            Some((
                format!("{name} {version}"),
                (external, block.trim().to_owned()),
            ))
        })
        .collect()
}

#[must_use]
pub fn lock_delta(before: &str, after: &str) -> LockDelta {
    let (old, new) = (lock_packages(before), lock_packages(after));
    let mut delta = LockDelta::default();
    for key in old.keys().chain(new.keys()) {
        if old.get(key) == new.get(key) {
            continue;
        }
        let external = old.get(key).or(new.get(key)).is_some_and(|(e, _)| *e);
        if external {
            delta.external = true;
        } else if let Some((name, _)) = key.split_once(' ') {
            delta.workspace.insert(name.to_owned());
        }
    }
    delta
}

/// A `Cargo.lock` change that only touches workspace crates (a tool added, a crate version bumped)
/// is not a global change: it counts as a change of those crates alone. Any external dependency
/// change stays global (it can alter every bundle).
fn refine_lock(files: &mut Vec<String>, base: &str, head: &str, krates: &[crate::changed::Krate]) {
    if !files.iter().any(|f| f == "Cargo.lock") {
        return;
    }
    let show = |rev: &str| {
        std::process::Command::new("git")
            .args(["show", &format!("{rev}:Cargo.lock")])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let (Some(before), Some(after)) = (show(base), show(head)) else {
        return;
    };
    let delta = lock_delta(&before, &after);
    if delta.external {
        return;
    }
    files.retain(|f| f != "Cargo.lock");
    for name in &delta.workspace {
        if let Some(krate) = krates.iter().find(|k| &k.name == name) {
            files.push(format!("{}/Cargo.toml", krate.dir));
        }
    }
}

/// `cargo xtask feed-plan --previous-dir D [--previous-channel C] [--force-all] [--force-cli] [--head REF]`: reads the
/// published signed index in `D` (`index.json` + `index.json.sig`), refuses anything not signed by
/// the release key, and prints what to build as `key=value` lines and `.kortex/feed-plan.json`.
pub fn plan(args: &[String]) -> Result<(), String> {
    let dir =
        PathBuf::from(flag(args, "--previous-dir").ok_or("usage : --previous-dir <dossier>")?);
    // The test channel plans against the published stable index (what is really in production).
    let channel = flag(args, "--previous-channel").unwrap_or_else(|| "stable".into());
    let bytes = fs::read(dir.join("index.json")).map_err(|e| format!("index.json : {e}"))?;
    let sig = fs::read_to_string(dir.join("index.json.sig"))
        .map_err(|e| format!("index.json.sig : {e}"))?;
    let public =
        fs::read_to_string("crates/kortex-catalog/release.pub").map_err(|e| e.to_string())?;
    let (previous, _) = kortex_pkg::check_index(&bytes, &sig, public.trim(), &channel, None)
        .map_err(|e| format!("l'index publié est refusé : {e}"))?;
    let krates = crate::changed::workspace(&std::env::current_dir().map_err(|e| e.to_string())?)?;
    let head = flag(args, "--head").unwrap_or_else(|| "HEAD".into());
    let files = previous.source_commit.as_deref().and_then(|base| {
        let mut files = crate::changed::git_files_between(base, &head).ok()?;
        refine_lock(&mut files, base, &head, &krates);
        Some(files)
    });
    let versions = current_versions()?;
    let plan = decide(
        &previous,
        files.as_deref(),
        &krates,
        &versions,
        &web_ids()?,
        args.iter().any(|a| a == "--force-all"),
    );
    // `--force-cli`: publish the CLI although no file of its closure changed since the published
    // commit (a version bump of the CLI crate would touch Cargo.lock and so rebuild everything).
    let mut plan = plan;
    if args.iter().any(|a| a == "--force-cli") && !plan.cli {
        plan.cli = true;
        if let (Some(now), Some(published)) = (versions.get("cli"), &previous.cli)
            && !is_newer(now, &published.version)
        {
            plan.unbumped
                .push(format!("cli ({now}, publiée : {})", published.version));
        }
    }
    if !plan.unbumped.is_empty() {
        return Err(format!(
            "✗ modifié sans monter la version (corrige `version` dans tool.toml/app.toml/Cargo.toml de la CLI) :\n  {}",
            plan.unbumped.join("\n  ")
        ));
    }
    let list = |s: &BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>().join(" ");
    let result = json!({
        "reason": plan.reason,
        "nothing": plan.nothing(),
        "web_tools": list(&plan.web_tools),
        "web_apps": list(&plan.web_apps),
        "native_tools": list(&plan.native_tools),
        "native_apps": list(&plan.native_apps),
        "cli": plan.cli,
        "previous_sequence": previous.sequence,
    });
    fs::create_dir_all(".kortex").map_err(|e| e.to_string())?;
    fs::write(
        ".kortex/feed-plan.json",
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    for key in [
        "reason",
        "nothing",
        "web_tools",
        "web_apps",
        "native_tools",
        "native_apps",
        "cli",
        "previous_sequence",
    ] {
        let value = &result[key];
        println!(
            "{key}={}",
            value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_owned)
        );
    }
    Ok(())
}

fn bundle_dir(id: &str) -> PathBuf {
    if id.starts_with("apps.") {
        Path::new("dist/apps").join(id)
    } else {
        Path::new("dist/run").join(id)
    }
}

fn collect(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err(format!("lien symbolique refusé : {}", path.display()));
        }
        if meta.is_dir() {
            collect(&path, base, out)?;
        } else {
            out.push(
                path.strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_owned(),
            );
        }
    }
    Ok(())
}

/// `cargo xtask package-web <id…>`: `dist/release/web-<id>-<version>.tar.gz` holding the compiled
/// bundle (`dist/run/<id>` or `dist/apps/<id>`) under `web/`, plus a manifest.
pub fn package_web(args: &[String]) -> Result<(), String> {
    use flate2::{Compression, write::GzEncoder};
    if args.is_empty() {
        return Err("usage : cargo xtask package-web <id…>".into());
    }
    let versions = current_versions()?;
    fs::create_dir_all("dist/release").map_err(|e| e.to_string())?;
    for id in args {
        let version = versions
            .get(id)
            .ok_or_else(|| format!("`{id}` introuvable"))?;
        let dir = bundle_dir(id);
        if !dir.is_dir() {
            return Err(format!(
                "{} absent : compile `{id}` pour le web d'abord",
                dir.display()
            ));
        }
        let mut files = Vec::new();
        collect(&dir, &dir, &mut files)?;
        files.sort();
        let manifest = serde_json::to_vec_pretty(&Manifest {
            id: id.clone(),
            version: version.clone(),
            target: "web".into(),
            binary: "web".into(),
            sha256_binary: crate::build::tree_fingerprint(&dir)?,
        })
        .map_err(|e| e.to_string())?;
        let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::best()));
        let mut put = |name: &str, data: &[u8]| -> Result<(), String> {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_cksum();
            tar.append_data(&mut header, name, data)
                .map_err(|e| e.to_string())
        };
        put("manifest.json", &manifest)?;
        for relative in &files {
            let name = format!("web/{}", relative.to_string_lossy().replace('\\', "/"));
            put(
                &name,
                &fs::read(dir.join(relative)).map_err(|e| e.to_string())?,
            )?;
        }
        let bytes = tar
            .into_inner()
            .and_then(GzEncoder::finish)
            .map_err(|e| e.to_string())?;
        let out = format!("dist/release/web-{id}-{version}.tar.gz");
        fs::write(&out, &bytes).map_err(|e| e.to_string())?;
        println!(
            "✓ {out} ({} fichier(s), {} Ko)",
            files.len(),
            bytes.len() / 1024
        );
    }
    Ok(())
}

/// `cargo xtask restore-web --index <index.json|url>`: rebuilds `dist/run` and `dist/apps` from the
/// `web` archives of a signed index (their SHA-256 and size are checked against it).
pub fn restore_web(args: &[String]) -> Result<(), String> {
    let source = flag(args, "--index").ok_or("usage : --index <index.json ou URL>")?;
    let index: Index =
        serde_json::from_slice(&kortex_pkg::fetch(&source, 8 << 20).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let mut restored = 0_usize;
    for (id, release) in &index.tools {
        let Some(artifact) = release.targets.get("web") else {
            continue;
        };
        if !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        {
            return Err(format!("identifiant refusé : {id}"));
        }
        // Archives are immutable and named by their signed SHA-256: keep them, a publication that
        // changes one tool then downloads one archive, not all of them.
        let cached = PathBuf::from(".kortex/web-cache").join(&artifact.sha256);
        let bytes = match fs::read(&cached) {
            Ok(bytes)
                if bytes.len() as u64 == artifact.size && sha256_hex(&bytes) == artifact.sha256 =>
            {
                bytes
            }
            _ => {
                let url = kortex_pkg::resolve(&source, &artifact.url);
                let bytes =
                    kortex_pkg::fetch(&url, 256 << 20).map_err(|e| format!("{id} : {e}"))?;
                if bytes.len() as u64 != artifact.size || sha256_hex(&bytes) != artifact.sha256 {
                    return Err(format!(
                        "{id} : l'archive web ne correspond pas à l'index signé"
                    ));
                }
                if fs::create_dir_all(".kortex/web-cache").is_ok() {
                    let _ = fs::write(&cached, &bytes);
                }
                bytes
            }
        };
        let dest = bundle_dir(id);
        let _ = fs::remove_dir_all(&dest);
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()));
        for entry in tar.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let path = entry.path().map_err(|e| e.to_string())?.into_owned();
            let Ok(relative) = path.strip_prefix("web") else {
                continue;
            };
            if relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err(format!("{id} : chemin refusé {}", path.display()));
            }
            let to = dest.join(relative);
            fs::create_dir_all(to.parent().ok_or("chemin invalide")?).map_err(|e| e.to_string())?;
            let mut file = fs::File::create(&to).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
        }
        if id.starts_with("apps.") {
            // The publisher checks every app file against a receipt; the archive itself was already
            // verified against the signed index, so the receipt is derived from the extracted tree.
            let mut files = Vec::new();
            collect(&dest, &dest, &mut files)?;
            let mut hashes = serde_json::Map::new();
            for relative in files {
                let bytes = fs::read(dest.join(&relative)).map_err(|e| e.to_string())?;
                hashes.insert(
                    relative.to_string_lossy().replace('\\', "/"),
                    sha256_hex(&bytes).into(),
                );
            }
            fs::create_dir_all(".kortex/app-receipts").map_err(|e| e.to_string())?;
            fs::write(
                format!(".kortex/app-receipts/{id}-web.json"),
                json!({"target": "web", "files": hashes}).to_string(),
            )
            .map_err(|e| e.to_string())?;
        }
        restored += 1;
    }
    if restored == 0 {
        return Err("aucune archive web dans l'index".into());
    }
    println!("✓ {restored} bundle(s) web restauré(s) (dist/run, dist/apps)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::changed::Krate;
    use kortex_pkg::{Artifact, ToolRelease};
    use std::collections::BTreeMap;

    fn krate(name: &str, dir: &str, deps: &[&str]) -> Krate {
        Krate {
            name: name.into(),
            dir: dir.into(),
            deps: deps.iter().map(|d| (*d).to_owned()).collect(),
        }
    }

    fn workspace() -> Vec<Krate> {
        vec![
            krate("kortex-runtime", "crates/kortex-runtime", &[]),
            krate("kortex-ui", "crates/kortex-ui", &[]),
            krate("kortex-platform", "crates/kortex-platform", &[]),
            krate("kortex-contract", "crates/kortex-contract", &[]),
            krate("kortex-i18n", "crates/kortex-i18n", &[]),
            krate(
                "kortex-player",
                "crates/kortex-player",
                &[
                    "kortex-ui",
                    "kortex-runtime",
                    "kortex-platform",
                    "kortex-contract",
                    "kortex-i18n",
                ],
            ),
            krate("kortex-pkg", "crates/kortex-pkg", &[]),
            krate("kortex-cli", "crates/kortex-cli", &["kortex-pkg"]),
            krate("tool-text-a", "tools/text/a", &[]),
            krate("tool-text-b", "tools/text/b", &[]),
            krate(
                "kortex-store",
                "apps/store",
                &["kortex-ui", "kortex-platform"],
            ),
        ]
    }

    fn entry(version: &str, web: bool) -> ToolRelease {
        let artifact = |u: &str| Artifact {
            url: u.into(),
            sha256: "00".into(),
            size: 1,
        };
        let mut targets =
            BTreeMap::from([("x86_64-apple-darwin".to_owned(), artifact("https://x/y"))]);
        if web {
            targets.insert("web".to_owned(), artifact("https://x/w"));
        }
        ToolRelease {
            name: BTreeMap::new(),
            universe: "text".into(),
            version: version.into(),
            targets,
        }
    }

    fn index(web: bool) -> Index {
        Index {
            schema: 1,
            channel: "stable".into(),
            sequence: 3,
            source_commit: Some("abc".into()),
            generated: "2026-10-20".into(),
            cli: None,
            tools: BTreeMap::from([
                ("text.a".to_owned(), entry("1.0.0", web)),
                ("text.b".to_owned(), entry("1.0.0", web)),
                ("apps.store".to_owned(), entry("0.3.0", web)),
            ]),
        }
    }

    fn versions(a: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("text.a".to_owned(), a.to_owned()),
            ("text.b".to_owned(), "1.0.0".to_owned()),
            ("apps.store".to_owned(), "0.3.0".to_owned()),
        ])
    }

    fn ids() -> (Vec<String>, Vec<String>) {
        (
            vec!["text.a".into(), "text.b".into()],
            vec!["apps.store".into()],
        )
    }

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|f| (*f).to_owned()).collect()
    }

    #[test]
    fn a_single_tool_change_builds_only_that_tool_and_leaves_apps_and_cli_alone() {
        let f = files(&["tools/text/a/src/engine.rs", "tools/text/a/tool.toml"]);
        let p = decide(
            &index(true),
            Some(&f),
            &workspace(),
            &versions("1.0.1"),
            &ids(),
            false,
        );
        assert_eq!(p.web_tools, BTreeSet::from(["text.a".to_owned()]));
        assert_eq!(p.native_tools, p.web_tools);
        assert!(p.web_apps.is_empty() && p.native_apps.is_empty() && !p.cli);
        assert!(p.unbumped.is_empty());
    }

    #[test]
    fn a_changed_tool_without_a_version_bump_is_reported() {
        let f = files(&["tools/text/a/src/engine.rs"]);
        let p = decide(
            &index(true),
            Some(&f),
            &workspace(),
            &versions("1.0.0"),
            &ids(),
            false,
        );
        assert_eq!(p.unbumped.len(), 1, "{p:?}");
        assert!(p.unbumped[0].starts_with("text.a"));
    }

    #[test]
    fn a_shared_crate_rebuilds_everything_it_reaches() {
        let f = files(&["crates/kortex-platform/src/lib.rs"]);
        let p = decide(
            &index(true),
            Some(&f),
            &workspace(),
            &versions("1.0.1"),
            &ids(),
            false,
        );
        assert_eq!(p.web_tools.len(), 2, "{p:?}");
        assert_eq!(p.web_apps, BTreeSet::from(["apps.store".to_owned()]));
        assert!(
            p.unbumped.iter().any(|u| u.starts_with("text.b")),
            "b is rebuilt, so it needs a new version"
        );
    }

    #[test]
    fn docs_and_the_registry_publish_nothing() {
        let f = files(&[
            "docs/PLAN.md",
            "registry/contracts.json",
            "web/site/src/pages/index.astro",
        ]);
        let p = decide(
            &index(true),
            Some(&f),
            &workspace(),
            &versions("1.0.0"),
            &ids(),
            false,
        );
        assert!(p.nothing(), "{p:?}");
    }

    #[test]
    fn the_first_run_builds_every_bundle_for_the_web_only() {
        let p = decide(
            &index(false),
            Some(&files(&["docs/PLAN.md"])),
            &workspace(),
            &versions("1.0.0"),
            &ids(),
            false,
        );
        assert_eq!(p.web_tools.len(), 2);
        assert_eq!(p.web_apps.len(), 1);
        assert!(
            p.native_tools.is_empty() && p.native_apps.is_empty() && !p.cli,
            "native packages are reused"
        );
        // A tool the published index never listed still gets its desktop packages.
        let mut with_new = ids();
        with_new.0.push("text.new".into());
        let fresh = decide(
            &index(false),
            Some(&files(&["docs/PLAN.md"])),
            &workspace(),
            &versions("1.0.0"),
            &with_new,
            false,
        );
        assert_eq!(fresh.native_tools, BTreeSet::from(["text.new".to_owned()]));
        assert!(p.unbumped.is_empty());
        let unknown = decide(
            &index(true),
            None,
            &workspace(),
            &versions("1.0.0"),
            &ids(),
            false,
        );
        assert_eq!(
            unknown.web_tools.len(),
            2,
            "unknown base commit: rebuild the web"
        );
    }

    #[test]
    fn a_new_tool_in_the_lockfile_is_not_a_global_change() {
        let before = "[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+x\"\n\n[[package]]\nname = \"tool-text-a\"\nversion = \"0.1.0\"\n";
        let added = format!(
            "{before}\n[[package]]\nname = \"tool-text-new\"\nversion = \"0.1.0\"\ndependencies = [\"serde\"]\n"
        );
        let d = lock_delta(before, &added);
        assert_eq!(d.workspace, BTreeSet::from(["tool-text-new".to_owned()]));
        assert!(!d.external);
        // A crate version bump only concerns that crate.
        let bumped = before.replace(
            "tool-text-a\"\nversion = \"0.1.0",
            "tool-text-a\"\nversion = \"0.1.1",
        );
        assert_eq!(
            lock_delta(before, &bumped).workspace,
            BTreeSet::from(["tool-text-a".to_owned()])
        );
        // An external dependency change stays global.
        let upgraded = before.replace("1.0.0", "1.0.1");
        assert!(lock_delta(before, &upgraded).external);
    }

    #[test]
    fn a_brand_new_tool_is_added_without_rebuilding_anything_else() {
        let mut with_new = ids();
        with_new.0.push("text.new".into());
        let f = files(&["tools/text/new/src/engine.rs", "tools/text/new/tool.toml"]);
        let p = decide(
            &index(true),
            Some(&f),
            &workspace(),
            &versions("1.0.0"),
            &with_new,
            false,
        );
        assert_eq!(
            p.web_tools,
            BTreeSet::from(["text.new".to_owned()]),
            "{p:?}"
        );
        assert_eq!(p.native_tools, p.web_tools);
        assert!(p.web_apps.is_empty() && !p.cli && p.unbumped.is_empty());
    }

    #[test]
    fn a_cli_only_change_builds_the_cli_alone() {
        let f = files(&["crates/kortex-cli/src/lib.rs"]);
        let mut v = versions("1.0.0");
        v.insert("cli".into(), "0.2.0".into());
        let mut previous = index(true);
        previous.cli = Some(kortex_pkg::CliRelease {
            version: "0.1.0".into(),
            targets: BTreeMap::new(),
        });
        let p = decide(&previous, Some(&f), &workspace(), &v, &ids(), false);
        assert!(
            p.cli && p.web_tools.is_empty() && p.native_tools.is_empty(),
            "{p:?}"
        );
        assert!(p.unbumped.is_empty());
    }
}
