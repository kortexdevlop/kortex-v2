//! Installable web apps: manifest, icons and offline service worker written next to the compiled
//! bundle by `cargo xtask build-app <id> --web`. Colors come from the design tokens, the name and
//! description from the app contract, the file inventory from the build output itself.
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::apps::App;
use crate::fwd;

const TEMPLATE: &str = include_str!("../../web/shell/sw.template.js");

/// Light-theme `bg` and `accent` tokens (`#rrggbb`).
fn tokens() -> Result<(String, String), String> {
    let text = fs::read_to_string("design/tokens/colors.toml").map_err(|e| e.to_string())?;
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let light = table
        .get("light")
        .and_then(toml::Value::as_table)
        .ok_or("colors.toml : [light] manquant")?;
    let get = |name: &str| {
        light
            .get(name)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("colors.toml : light.{name} manquant"))
    };
    Ok((get("bg")?, get("accent")?))
}

fn files(dir: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            files(&path, found)?;
        } else {
            found.push(path);
        }
    }
    Ok(())
}

/// The web app manifest (`lang` of the root page; `/en/` carries the same shell).
#[must_use]
pub fn manifest(contract: &kortex_contract::AppContract, background: &str, theme: &str) -> String {
    let text = |map: &std::collections::BTreeMap<String, String>, lang: &str| {
        map.get(lang)
            .or_else(|| map.get("en"))
            .cloned()
            .unwrap_or_default()
    };
    let name = text(&contract.app.name, "fr");
    serde_json::to_string_pretty(&json!({
        "name": name,
        "short_name": name.trim_start_matches("Kortex ").to_owned(),
        "description": text(&contract.app.tagline, "fr"),
        "lang": "fr",
        "id": "/",
        "start_url": "/",
        "scope": "/",
        "display": "standalone",
        "background_color": background,
        "theme_color": theme,
        "icons": [
            {"src": "/icons/icon-192.png", "sizes": "192x192", "type": "image/png", "purpose": "any"},
            {"src": "/icons/icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any"},
        ],
    }))
    .unwrap_or_default()
        + "\n"
}

/// The service worker for the given public inventory (paths relative to the site root). Files
/// under `on_demand` prefixes are not precached: they are cached when first requested.
#[must_use]
pub fn service_worker(id: &str, inventory: &[(String, Vec<u8>)], on_demand: &[&str]) -> String {
    let mut hash = DefaultHasher::new();
    TEMPLATE.hash(&mut hash);
    let mut assets: Vec<String> = Vec::new();
    for (relative, bytes) in inventory {
        relative.hash(&mut hash);
        bytes.hash(&mut hash);
        let url = format!("/{relative}");
        if !on_demand.iter().any(|p| url.starts_with(p)) {
            assets.push(url);
        }
    }
    assets.extend(["/".to_owned(), "/en/".to_owned()]);
    let prefix = format!("kortex-app:{id}:");
    let json = |v: &str| serde_json::to_string(v).unwrap_or_default();
    TEMPLATE
        .replace(
            "__CACHE__",
            &json(&format!("{prefix}{:016x}", hash.finish())),
        )
        .replace("__PREFIX__", &json(&prefix))
        .replace(
            "__ASSETS__",
            &serde_json::to_string(&assets).unwrap_or_default(),
        )
        .replace(
            "__ON_DEMAND__",
            &serde_json::to_string(on_demand).unwrap_or_default(),
        )
}

/// Writes `manifest.webmanifest`, `icons/*.png` and `sw.js` into the compiled app directory.
///
/// # Errors
/// A token, the glyph or a file could not be read or written.
pub fn write(app: &App, destination: &Path) -> Result<(), String> {
    let (background, accent) = tokens()?;
    let glyph =
        fs::read_to_string(app.dir.join("glyph.svg")).map_err(|e| format!("glyph.svg : {e}"))?;
    let icons = destination.join("icons");
    fs::create_dir_all(&icons).map_err(|e| e.to_string())?;
    for (name, png) in crate::icons::app_icons(&glyph, &accent)? {
        fs::write(icons.join(name), png).map_err(|e| e.to_string())?;
    }
    fs::write(
        destination.join("manifest.webmanifest"),
        manifest(&app.contract, &background, &accent),
    )
    .map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    files(destination, &mut found)?;
    found.sort();
    let mut inventory = Vec::new();
    for path in found {
        let relative = fwd(path.strip_prefix(destination).map_err(|e| e.to_string())?);
        if relative != "sw.js" {
            inventory.push((relative, fs::read(&path).map_err(|e| e.to_string())?));
        }
    }
    let on_demand: &[&str] = if app.contract.app.bundles_engines {
        &["/engines/"]
    } else {
        &[]
    };
    let id = app.contract.app.id.trim_start_matches("apps.");
    fs::write(
        destination.join("sw.js"),
        service_worker(id, &inventory, on_demand),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> kortex_contract::AppContract {
        let text = include_str!("../../apps/mind/app.toml");
        kortex_contract::parse_app(text).unwrap_or_else(|e| unreachable!("{e:?}"))
    }

    #[test]
    fn the_manifest_is_installable_and_uses_the_contract_and_tokens() {
        let manifest: serde_json::Value =
            serde_json::from_str(&super::manifest(&contract(), "#ffffff", "#0a84ff"))
                .unwrap_or_default();
        assert_eq!(manifest["name"], "Kortex Mind");
        assert_eq!(manifest["short_name"], "Mind");
        assert_eq!(manifest["display"], "standalone");
        assert_eq!(
            (manifest["start_url"].as_str(), manifest["scope"].as_str()),
            (Some("/"), Some("/"))
        );
        assert_eq!(manifest["theme_color"], "#0a84ff");
        let sizes: Vec<&str> = manifest["icons"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| i["sizes"].as_str())
            .collect();
        assert_eq!(
            sizes,
            ["192x192", "512x512"],
            "Chrome needs 192 and 512 for installability"
        );
        assert!(
            manifest["description"]
                .as_str()
                .is_some_and(|d| !d.is_empty())
        );
    }

    #[test]
    fn the_worker_precaches_the_shell_but_not_on_demand_engines_nor_itself() {
        let inventory = vec![
            ("index.html".to_owned(), b"<html>".to_vec()),
            ("assets/app.js".to_owned(), b"js".to_vec()),
            (
                "engines/text.cleaner/engine_bg.wasm".to_owned(),
                vec![0, 1, 2],
            ),
        ];
        let worker = service_worker("mind", &inventory, &["/engines/"]);
        assert!(worker.contains(r#""/index.html""#) && worker.contains(r#""/assets/app.js""#));
        assert!(
            worker.contains(r#""/""#) && worker.contains(r#""/en/""#),
            "both language shells"
        );
        assert!(
            !worker.contains("engine_bg.wasm\"")
                || worker.contains(r#"const ON_DEMAND = ["/engines/"]"#)
        );
        let assets_line = worker
            .lines()
            .find(|l| l.starts_with("const ASSETS"))
            .unwrap_or_default();
        assert!(
            !assets_line.contains("engines") && !assets_line.contains("sw.js"),
            "{assets_line}"
        );
        assert!(worker.contains(r#"const PREFIX = "kortex-app:mind:""#));
        assert!(!worker.contains("__"), "every placeholder is substituted");
    }

    #[test]
    fn the_cache_name_changes_with_any_file() {
        let a = service_worker("x", &[("index.html".into(), b"1".to_vec())], &[]);
        let b = service_worker("x", &[("index.html".into(), b"2".to_vec())], &[]);
        let cache = |w: &str| {
            w.lines()
                .find(|l| l.starts_with("const CACHE"))
                .unwrap_or_default()
                .to_owned()
        };
        assert_ne!(cache(&a), cache(&b));
        assert_eq!(
            cache(&a),
            cache(&service_worker(
                "x",
                &[("index.html".into(), b"1".to_vec())],
                &[]
            ))
        );
    }

    #[test]
    fn the_accent_hue_comes_from_the_token() {
        let hue = crate::icons::hue_of_hex("#0a84ff").unwrap_or(0.0);
        assert!((240.0..275.0).contains(&hue), "azure is blue: {hue}");
        let red = crate::icons::hue_of_hex("#ff0000").unwrap_or(0.0);
        assert!((20.0..40.0).contains(&red), "{red}");
        assert_eq!(crate::icons::hue_of_hex("0a84ff"), None);
        assert_eq!(crate::icons::hue_of_hex("#12"), None);
    }
}
