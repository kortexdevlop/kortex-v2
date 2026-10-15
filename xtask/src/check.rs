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
        let expected = format!("tools/{}/", contract.tool.universe);
        if !dir.starts_with(&expected) {
            errors.push(format!("✗ {dir} : doit se trouver dans `{expected}<nom>/`"));
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
        let current = fs::read_to_string("registry/index.json").unwrap_or_default();
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
        println!("✓ check OK ({count} outil(s))");
        Ok(())
    } else {
        Err(format!(
            "{}\n{} problème(s)",
            errors.join("\n"),
            errors.len()
        ))
    }
}
