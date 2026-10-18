//! `cargo xtask changed [--base <ref>] [--files <path…>]`: which web bundles a change affects.
//!
//! Changed files are mapped to workspace crates, then to every tool/app whose dependency closure
//! contains one of them (a tool bundle also links the host wrapper libraries, see
//! [`WRAPPER_CRATES`]). Files outside any crate (toolchain, lockfile, design tokens, bundle
//! generators) invalidate everything: it is better to rebuild too much than to publish a stale
//! artifact (RULES 7.3).
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

/// Libraries every generated tool wrapper links besides the tool crate itself.
const WRAPPER_CRATES: [&str; 5] = [
    "kortex-ui",
    "kortex-runtime",
    "kortex-platform",
    "kortex-contract",
    "kortex-i18n",
];

/// Paths whose change can alter every bundle.
pub const GLOBAL_PATHS: [&str; 12] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo/",
    "design/",
    "brand/",
    "xtask/Cargo.toml",
    "xtask/src/build.rs",
    "xtask/src/worker.rs",
    "xtask/src/platform_check.rs",
    "xtask/src/tools.rs",
    "xtask/src/apps.rs",
];

/// Workspace crate: its directory (forward slashes, relative to the root) and path dependencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Krate {
    pub name: String,
    pub dir: String,
    pub deps: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Affected {
    pub all: bool,
    pub tools: BTreeSet<String>,
    pub apps: BTreeSet<String>,
}

/// Reads `cargo metadata --no-deps` output; `root` is the workspace root with a trailing `/`.
pub fn parse_metadata(json: &str, root: &str) -> Result<Vec<Krate>, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let packages = value["packages"]
        .as_array()
        .ok_or("metadata sans packages")?;
    let names: BTreeSet<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
    let mut krates = Vec::new();
    for package in packages {
        let name = package["name"].as_str().ok_or("package sans nom")?;
        let manifest = package["manifest_path"].as_str().ok_or("manifest absent")?;
        let dir = manifest
            .trim_end_matches("/Cargo.toml")
            .strip_prefix(root)
            .unwrap_or_default()
            .to_owned();
        let deps = package["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| d["name"].as_str())
            .filter(|d| names.contains(d))
            .map(str::to_owned)
            .collect();
        krates.push(Krate {
            name: name.to_owned(),
            dir,
            deps,
        });
    }
    Ok(krates)
}

fn closure(start: &str, graph: &BTreeMap<&str, &Krate>) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![start.to_owned()];
    while let Some(name) = stack.pop() {
        if seen.insert(name.clone())
            && let Some(krate) = graph.get(name.as_str())
        {
            stack.extend(krate.deps.iter().cloned());
        }
    }
    seen
}

/// Workspace crates read from `cargo metadata` (path dependencies only).
///
/// # Errors
/// `cargo metadata` failed or produced unreadable output.
pub fn workspace(root: &std::path::Path) -> Result<Vec<Krate>, String> {
    // `abs(".")` yields `…/.`: normalise so manifest paths can be made relative to the root.
    let root: std::path::PathBuf = root.components().collect();
    let root = root.as_path();
    let out = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata : {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    parse_metadata(
        &String::from_utf8_lossy(&out.stdout),
        &format!("{}/", root.to_string_lossy()),
    )
}

/// Workspace crates whose sources end up in the bundle built from the crate in `dir`: its own
/// dependency closure, plus the host wrapper libraries for a tool (`tools/…`). One rule shared
/// by PR selection ([`affected`]) and the build receipts, so they can never disagree.
#[must_use]
pub fn bundle_crates<'a>(krates: &'a [Krate], dir: &str) -> Vec<&'a Krate> {
    let graph: BTreeMap<&str, &Krate> = krates.iter().map(|k| (k.name.as_str(), k)).collect();
    let Some(own) = krates.iter().find(|k| k.dir == dir) else {
        return Vec::new();
    };
    let mut names = closure(&own.name, &graph);
    if dir.starts_with("tools/") {
        for wrapper in WRAPPER_CRATES {
            names.extend(closure(wrapper, &graph));
        }
    }
    names
        .iter()
        .filter_map(|n| graph.get(n.as_str()).copied())
        .collect()
}

#[must_use]
pub fn affected(files: &[String], krates: &[Krate]) -> Affected {
    let mut result = Affected::default();
    let mut changed: BTreeSet<&str> = BTreeSet::new();
    let mut registry_changed = false;
    for file in files {
        if GLOBAL_PATHS
            .iter()
            .any(|p| file == p || (p.ends_with('/') && file.starts_with(p)))
        {
            result.all = true;
        }
        if file.starts_with("registry/")
            || file.starts_with("web/shell/")
            || file == "xtask/src/pwa.rs"
            || file == "xtask/src/icons.rs"
        {
            registry_changed = true;
        }
        if let Some(owner) = krates
            .iter()
            .filter(|k| !k.dir.is_empty() && file.starts_with(&format!("{}/", k.dir)))
            .max_by_key(|k| k.dir.len())
        {
            changed.insert(owner.name.as_str());
        }
    }
    for krate in krates {
        let is_tool = krate.dir.starts_with("tools/") && !krate.dir.ends_with("_template");
        let is_app = krate.dir.starts_with("apps/");
        if !is_tool && !is_app {
            continue;
        }
        let deps: BTreeSet<&str> = bundle_crates(krates, &krate.dir)
            .iter()
            .map(|k| k.name.as_str())
            .collect();
        let hit =
            result.all || (is_app && registry_changed) || deps.iter().any(|d| changed.contains(d));
        if !hit {
            continue;
        }
        if is_tool {
            if let Some(rest) = krate.name.strip_prefix("tool-")
                && let Some((universe, name)) =
                    krate.dir.rsplit_once('/').and_then(|(parent, n)| {
                        parent
                            .rsplit_once('/')
                            .map(|(_, u)| (u.to_owned(), n.to_owned()))
                    })
            {
                let _ = rest;
                result.tools.insert(format!("{universe}.{name}"));
            }
        } else if let Some(name) = krate.dir.strip_prefix("apps/") {
            result.apps.insert(format!("apps.{name}"));
        }
    }
    result
}

fn git_files(base: &str) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .args(["diff", "--name-only", &format!("{base}...HEAD")])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "git diff {base}...HEAD : {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

pub fn run(args: &[String]) -> Result<(), String> {
    let files = if let Some(i) = args.iter().position(|a| a == "--files") {
        args[i + 1..].to_vec()
    } else {
        let base = args
            .iter()
            .position(|a| a == "--base")
            .and_then(|i| args.get(i + 1))
            .ok_or("usage : cargo xtask changed --base <ref> | --files <chemin…>")?;
        git_files(base)?
    };
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let out = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata : {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let krates = parse_metadata(
        &String::from_utf8_lossy(&out.stdout),
        &format!("{}/", root.to_string_lossy()),
    )?;
    let result = affected(&files, &krates);
    // `key=value` lines: append them to $GITHUB_OUTPUT as is.
    println!("all={}", result.all);
    println!(
        "tools={}",
        result.tools.iter().cloned().collect::<Vec<_>>().join(" ")
    );
    println!(
        "apps={}",
        result.apps.iter().cloned().collect::<Vec<_>>().join(" ")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, dir: &str, deps: &[&str]) -> Krate {
        Krate {
            name: name.into(),
            dir: dir.into(),
            deps: deps.iter().map(|d| (*d).to_owned()).collect(),
        }
    }

    fn workspace() -> Vec<Krate> {
        vec![
            krate("kortex-ui", "crates/kortex-ui", &["kortex-runtime"]),
            krate("kortex-runtime", "crates/kortex-runtime", &[]),
            krate("kortex-platform", "crates/kortex-platform", &[]),
            krate("kortex-contract", "crates/kortex-contract", &[]),
            krate("kortex-i18n", "crates/kortex-i18n", &[]),
            krate(
                "kortex-cli",
                "crates/kortex-cli",
                &["tool-text-a", "tool-calc-b"],
            ),
            krate("kortex", "crates/kortex", &["kortex-runtime"]),
            krate("tool-text-a", "tools/text/a", &["kortex"]),
            krate(
                "tool-calc-b",
                "tools/calc/b",
                &["kortex", "kortex-calendar"],
            ),
            krate("kortex-calendar", "crates/kortex-calendar", &[]),
            krate("kortex-store", "apps/store", &["kortex-ui"]),
            krate("kortex-pkg", "crates/kortex-pkg", &[]),
        ]
    }

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|f| (*f).to_owned()).collect()
    }

    #[test]
    fn a_tool_change_rebuilds_only_that_tool() {
        let r = affected(&files(&["tools/text/a/src/engine.rs"]), &workspace());
        assert!(!r.all && r.apps.is_empty());
        assert_eq!(r.tools, BTreeSet::from(["text.a".to_owned()]));
    }

    #[test]
    fn a_wrapper_library_change_rebuilds_every_tool_and_the_apps_using_it() {
        let r = affected(&files(&["crates/kortex-runtime/src/lib.rs"]), &workspace());
        assert_eq!(r.tools.len(), 2);
        assert_eq!(r.apps, BTreeSet::from(["apps.store".to_owned()]));
    }

    #[test]
    fn a_library_used_by_one_tool_only_is_targeted() {
        let r = affected(&files(&["crates/kortex-calendar/src/lib.rs"]), &workspace());
        assert_eq!(r.tools, BTreeSet::from(["calc.b".to_owned()]));
        assert!(r.apps.is_empty());
    }

    #[test]
    fn unrelated_crates_and_docs_rebuild_nothing() {
        let r = affected(
            &files(&["crates/kortex-pkg/src/lib.rs", "docs/PLAN.md"]),
            &workspace(),
        );
        assert_eq!(r, Affected::default());
    }

    #[test]
    fn toolchain_lockfile_tokens_and_generators_invalidate_everything() {
        for path in [
            "Cargo.lock",
            "design/tokens/colors.toml",
            "xtask/src/build.rs",
        ] {
            let r = affected(&files(&[path]), &workspace());
            assert!(r.all, "{path}");
            assert_eq!(r.tools.len(), 2, "{path}");
            assert_eq!(r.apps.len(), 1, "{path}");
        }
    }

    #[test]
    fn registry_and_web_shell_changes_only_touch_apps() {
        for path in ["registry/index.json", "web/shell/index.html"] {
            let r = affected(&files(&[path]), &workspace());
            assert!(r.tools.is_empty(), "{path}");
            assert_eq!(r.apps.len(), 1, "{path}");
        }
    }

    #[test]
    fn parses_cargo_metadata_keeping_workspace_path_dependencies_only() {
        let json = r#"{"packages":[
            {"name":"a","manifest_path":"/r/crates/a/Cargo.toml","dependencies":[{"name":"b"},{"name":"serde"}]},
            {"name":"b","manifest_path":"/r/crates/b/Cargo.toml","dependencies":[]}]}"#;
        let krates = parse_metadata(json, "/r/").unwrap_or_default();
        assert_eq!(krates[0], krate("a", "crates/a", &["b"]));
        assert_eq!(krates[1].dir, "crates/b");
    }
}
