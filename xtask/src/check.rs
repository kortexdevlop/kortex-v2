//! `cargo xtask check [tool-id]`: quality gates (messages in French, actionable).
use std::fs;

use kortex_contract::{LANGS, validate};

use crate::{registry, tools};

fn has_hex_color(line: &str) -> bool {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'#' {
            let n = b[i + 1..]
                .iter()
                .take_while(|c| c.is_ascii_hexdigit())
                .count();
            if matches!(n, 3 | 4 | 6 | 8) {
                return true;
            }
        }
        i += 1;
    }
    false
}

pub fn run(filter: Option<&str>) -> Result<(), String> {
    crate::platform_check::run()?;
    let universes = tools::universes()?;
    let mut errors: Vec<String> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_slugs = std::collections::HashSet::new();
    let all = tools::discover()?;
    let mut count = 0;

    for t in &all {
        let dir = t.dir.display().to_string();
        let contract = match &t.contract {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("✗ {dir} : tool.toml invalide — {e}"));
                continue;
            }
        };
        if filter.is_some_and(|f| f != contract.tool.id) {
            continue;
        }
        count += 1;
        let id = &contract.tool.id;
        for issue in validate(contract, &universes) {
            errors.push(format!("✗ {dir} : {}", issue.0));
        }
        let expected = std::path::Path::new("tools").join(&contract.tool.universe);
        if t.dir.parent() != Some(expected.as_path()) {
            errors.push(format!(
                "✗ {dir} : doit se trouver dans `tools/{}/<nom>/`",
                contract.tool.universe
            ));
        }
        if !seen_ids.insert(id.clone()) {
            errors.push(format!("✗ {dir} : id `{id}` en double"));
        }
        for lang in LANGS {
            if let Some(slug) = contract.tool.slug.get(lang)
                && !seen_slugs.insert((lang, slug.clone()))
            {
                errors.push(format!(
                    "✗ {dir} : slug {lang} `{slug}` déjà utilisé par un autre outil"
                ));
            }
        }
        for f in [
            "glyph.svg",
            "CHANGELOG.md",
            "src/lib.rs",
            "src/engine.rs",
            "locales/fr.toml",
            "locales/en.toml",
            "content/fr.md",
            "content/en.md",
            "tests/engine.rs",
        ] {
            if !t.dir.join(f).exists() {
                errors.push(format!(
                    "✗ {dir} : `{f}` manquant — voir docs/guides/add-tool.md"
                ));
            }
        }
        for issue in custom_ui_issues(
            contract.tool.ui == kortex_contract::UiKind::Custom,
            &fs::read_to_string(t.dir.join("Cargo.toml")).unwrap_or_default(),
            &fs::read_to_string(t.dir.join("src/lib.rs")).unwrap_or_default(),
            t.dir.join("src/ui.rs").is_file(),
        ) {
            errors.push(format!("✗ {dir} : {issue}"));
        }
        if let Ok(svg) = fs::read_to_string(t.dir.join("glyph.svg"))
            && !svg.contains("viewBox=\"0 0 24 24\"")
        {
            errors.push(format!(
                "✗ {dir} : glyph.svg doit avoir viewBox=\"0 0 24 24\""
            ));
        }
        for lang in LANGS {
            if let Ok(md) = fs::read_to_string(t.dir.join(format!("content/{lang}.md"))) {
                let h2 = md.lines().filter(|l| l.starts_with("## ")).count();
                let faq = md.lines().filter(|l| l.starts_with("### ")).count();
                if h2 < 3 {
                    errors.push(format!("✗ {dir} : content/{lang}.md doit avoir 3 sections `## ` (intro, étapes, FAQ)"));
                }
                if faq < 3 {
                    errors.push(format!(
                        "✗ {dir} : content/{lang}.md : FAQ ≥ 3 questions (`### question`)"
                    ));
                }
            }
        }
        if let (Ok(fr), Ok(en)) = (
            fs::read_to_string(t.dir.join("locales/fr.toml")),
            fs::read_to_string(t.dir.join("locales/en.toml")),
        ) {
            let keys = |s: &str| -> Result<Vec<String>, String> {
                let t: toml::Table = s.parse().map_err(|e: toml::de::Error| e.to_string())?;
                let mut v = Vec::new();
                for (k, val) in &t {
                    if let Some(tab) = val.as_table() {
                        v.extend(tab.keys().map(|kk| format!("{k}.{kk}")));
                    }
                }
                v.sort();
                Ok(v)
            };
            match (keys(&fr), keys(&en)) {
                (Ok(a), Ok(b)) if a != b => {
                    errors.push(format!("✗ {dir} : locales fr/en n'ont pas les mêmes clés"))
                }
                (Err(e), _) | (_, Err(e)) => {
                    errors.push(format!("✗ {dir} : locales invalides — {e}"))
                }
                _ => {}
            }
        }
        if let Ok(test) = fs::read_to_string(t.dir.join("tests/engine.rs"))
            && test.matches("#[test]").count() < 2
        {
            errors.push(format!(
                "✗ {dir} : tests/engine.rs : au moins un test nominal ET un test d'erreur"
            ));
        }
        if let Ok(entries) = fs::read_dir(t.dir.join("src")) {
            for e in entries.filter_map(Result::ok) {
                let Ok(src) = fs::read_to_string(e.path()) else {
                    continue;
                };
                for (n, line) in src.lines().enumerate() {
                    let where_ = format!("{}:{}", e.path().display(), n + 1);
                    if has_hex_color(line) {
                        errors.push(format!(
                            "✗ {where_} : couleur en dur — utilise les tokens (--k-*)"
                        ));
                    }
                    if line.contains("web_sys") || line.contains("std::fs") {
                        errors.push(format!(
                            "✗ {where_} : accès plateforme direct — passe par kortex-platform"
                        ));
                    }
                    if line.contains("tool_")
                        && line.contains("use ")
                        && !line.contains(&format!("tool_{}", id.replace('.', "_")))
                    {
                        errors.push(format!(
                            "✗ {where_} : un outil n'importe jamais un autre outil"
                        ));
                    }
                    if e.file_name() == "engine.rs"
                        && (line.contains(".unwrap()") || line.contains("panic!("))
                    {
                        errors.push(format!(
                            "✗ {where_} : unwrap/panic interdits dans un moteur"
                        ));
                    }
                }
            }
        }
    }

    if filter.is_none() {
        let current: Vec<_> = all.iter().filter_map(|t| t.contract.clone().ok()).collect();
        match crate::compat::baseline_registry() {
            Some((reference, registry)) => {
                match crate::compat::compare_registry(&registry, &current) {
                    Ok(issues) => errors.extend(
                        issues
                            .into_iter()
                            .map(|i| format!("✗ {i} (référence {reference})")),
                    ),
                    Err(e) => errors.push(format!("✗ référence {reference} : {e}")),
                }
            }
            None => println!(
                "· compatibilité des contrats ignorée : aucune référence git (définir KORTEX_BASELINE_REF)"
            ),
        }
    }

    if filter.is_none() {
        for app in crate::apps::discover()? {
            let dir = app.dir.display().to_string();
            for name in [
                "Cargo.toml",
                "app.toml",
                "src/lib.rs",
                "src/engine.rs",
                "glyph.svg",
                "locales/fr.toml",
                "locales/en.toml",
                "content/fr.md",
                "content/en.md",
                "CHANGELOG.md",
            ] {
                if !app.dir.join(name).is_file() {
                    errors.push(format!("✗ {dir} : {name} manquant"));
                }
            }
            if app.contract.app.ui == kortex_contract::UiKind::Custom
                && !app.dir.join("src/ui.rs").is_file()
            {
                errors.push(format!("✗ {dir} : ui=custom exige src/ui.rs"));
            }
            let locale_keys = |lang: &str| -> Result<std::collections::BTreeSet<String>, String> {
                let source = fs::read_to_string(app.dir.join(format!("locales/{lang}.toml")))
                    .map_err(|e| e.to_string())?;
                let table: toml::Table =
                    source.parse::<toml::Table>().map_err(|e| e.to_string())?;
                let mut keys = std::collections::BTreeSet::new();
                for (section, value) in table {
                    let values = value
                        .as_table()
                        .ok_or_else(|| format!("{section} doit être une section"))?;
                    for (key, value) in values {
                        if value.as_str().is_none_or(str::is_empty) {
                            return Err(format!("{section}.{key} doit être un texte non vide"));
                        }
                        keys.insert(format!("{section}.{key}"));
                    }
                }
                Ok(keys)
            };
            match (locale_keys("fr"), locale_keys("en")) {
                (Ok(fr), Ok(en)) if !fr.is_empty() && fr == en => {}
                _ => errors.push(format!(
                    "✗ {dir} : locales FR/EN invalides ou clés différentes"
                )),
            }
            if !app.dir.join("tests").is_dir() {
                errors.push(format!("✗ {dir} : tests manquants"));
            }
            let source = fs::read_to_string(app.dir.join("src/engine.rs")).unwrap_or_default();
            for forbidden in [
                "dioxus::",
                "web_sys",
                "std::fs",
                "std::process::Command",
                ".unwrap()",
                "panic!(",
            ] {
                if source.contains(forbidden) {
                    errors.push(format!(
                        "✗ {dir}/src/engine.rs : moteur non pur ({forbidden})"
                    ));
                }
            }
            fn reject_web_app(dir: &std::path::Path, errors: &mut Vec<String>) {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            reject_web_app(&path, errors);
                        } else if path
                            .extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| matches!(e, "ts" | "tsx" | "astro" | "jsx"))
                        {
                            errors.push(format!(
                                "✗ {} : application produit hors Rust/Dioxus",
                                path.display()
                            ));
                        }
                    }
                }
            }
            reject_web_app(&app.dir, &mut errors);
        }
        let brands: toml::Table = fs::read_to_string("brand/names.toml")
            .map_err(|e| e.to_string())?
            .parse::<toml::Table>()
            .map_err(|e| e.to_string())?;
        if let Some(names) = brands.get("apps").and_then(toml::Value::as_table) {
            for name in names.keys() {
                let mut letters = name.chars();
                let component = format!(
                    "{}{}.astro",
                    letters
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_default(),
                    letters.as_str()
                );
                let path = std::path::Path::new("web/site/src/components").join(component);
                if path.exists() {
                    errors.push(format!("✗ {} : une application doit être dans apps/ en Rust/Dioxus ; utiliser une intégration site générique", path.display()));
                }
            }
        }
        let app_registry = crate::apps::registry()?;
        if fs::read_to_string("registry/apps.json").unwrap_or_default() != app_registry {
            errors.push("✗ registry/apps.json périmé — lance cargo xtask registry".into());
        }
    }

    if filter.is_none() {
        let current = fs::read_to_string("registry/index.json").unwrap_or_default();
        let (cli_manifest, cli_code) = crate::cli_gen::build()?;
        if fs::read_to_string("crates/kortex-cli/Cargo.toml").unwrap_or_default() != cli_manifest
            || fs::read_to_string("crates/kortex-cli/src/generated.rs").unwrap_or_default()
                != cli_code
        {
            errors.push("✗ crates/kortex-cli périmé — lance `cargo xtask cli-gen`".into());
        }
        if current != registry::build()? {
            errors.push("✗ registry/index.json périmé — lance `cargo xtask registry`".into());
        }
    } else if count == 0 {
        errors.push(format!(
            "✗ outil `{}` introuvable",
            filter.unwrap_or_default()
        ));
    }

    if errors.is_empty() {
        println!(
            "✓ check OK ({count} outil(s), {} application(s))",
            if filter.is_none() {
                crate::apps::discover()?.len()
            } else {
                0
            }
        );
        Ok(())
    } else {
        Err(format!(
            "{}\n{} problème(s)",
            errors.join("\n"),
            errors.len()
        ))
    }
}

/// Rules linking `ui = "custom"` to the crate layout: the UI lives in `src/ui.rs` behind an
/// optional `ui` feature so headless builds (CLI, Worker engine) never link Dioxus.
fn custom_ui_issues(custom: bool, cargo: &str, lib: &str, has_ui_rs: bool) -> Vec<String> {
    let mut issues = Vec::new();
    if custom {
        if !has_ui_rs {
            issues.push(
                "ui = \"custom\" exige src/ui.rs (composant `Ui`, voir docs/guides/add-tool.md)"
                    .to_owned(),
            );
        }
        if !cargo.contains("\nui = [") {
            issues.push("ui = \"custom\" exige une feature `ui` dans Cargo.toml (dioxus et kortex-ui optionnels)".to_owned());
        }
        if !lib.contains("#[cfg(feature = \"ui\")]") || !lib.contains("pub mod ui;") {
            issues
                .push("src/lib.rs doit déclarer `#[cfg(feature = \"ui\")] pub mod ui;`".to_owned());
        }
    } else if has_ui_rs {
        issues.push(
            "src/ui.rs présent mais ui = \"auto\" : le déclarer custom ou le supprimer".to_owned(),
        );
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::custom_ui_issues;

    const CARGO: &str = "[features]\nui = [\"dep:dioxus\"]\n";
    const LIB: &str = "#[cfg(feature = \"ui\")]\npub mod ui;\n";

    #[test]
    fn complete_custom_layout_passes() {
        assert!(custom_ui_issues(true, CARGO, LIB, true).is_empty());
    }

    #[test]
    fn custom_without_ui_file_feature_or_gate_is_rejected() {
        assert_eq!(custom_ui_issues(true, "", "", false).len(), 3);
    }

    #[test]
    fn auto_tool_must_not_carry_an_unused_ui_file() {
        assert_eq!(custom_ui_issues(false, "", "", true).len(), 1);
        assert!(custom_ui_issues(false, "", "", false).is_empty());
    }
}
