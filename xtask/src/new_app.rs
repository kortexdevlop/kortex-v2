//! `cargo xtask new-app --name <nom>`: scaffolds `apps/<nom>/` from `xtask/templates/app`.
//! The same Rust/Dioxus sources are compiled for web and desktop (features `web` / `desktop`).
use std::fs;
use std::path::Path;

const FILES: &[(&str, &str)] = &[
    ("app.toml", include_str!("../templates/app/app.toml")),
    ("Cargo.toml", include_str!("../templates/app/Cargo.toml")),
    ("Dioxus.toml", include_str!("../templates/app/Dioxus.toml")),
    ("glyph.svg", include_str!("../templates/app/glyph.svg")),
    (
        "CHANGELOG.md",
        include_str!("../templates/app/CHANGELOG.md"),
    ),
    ("src/lib.rs", include_str!("../templates/app/src/lib.rs")),
    ("src/main.rs", include_str!("../templates/app/src/main.rs")),
    (
        "src/engine.rs",
        include_str!("../templates/app/src/engine.rs"),
    ),
    ("src/ui.rs", include_str!("../templates/app/src/ui.rs")),
    (
        "locales/fr.toml",
        include_str!("../templates/app/locales/fr.toml"),
    ),
    (
        "locales/en.toml",
        include_str!("../templates/app/locales/en.toml"),
    ),
    (
        "content/fr.md",
        include_str!("../templates/app/content/fr.md"),
    ),
    (
        "content/en.md",
        include_str!("../templates/app/content/en.md"),
    ),
    (
        "tests/engine.rs",
        include_str!("../templates/app/tests/engine.rs"),
    ),
];

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn pretty(name: &str) -> String {
    let mut chars = name.replace('-', " ").chars().collect::<Vec<_>>();
    if let Some(first) = chars.first_mut() {
        *first = first.to_ascii_uppercase();
    }
    chars.into_iter().collect()
}

/// Renders every template file for `name`; pure so it can be tested without touching the disk.
pub fn render(name: &str) -> Vec<(String, String)> {
    let vars = [
        ("name", name.to_owned()),
        ("Name", pretty(name)),
        ("crate", format!("kortex_{}", name.replace('-', "_"))),
    ];
    FILES
        .iter()
        .map(|(path, template)| {
            let mut text = (*template).to_owned();
            for (key, value) in &vars {
                text = text.replace(&format!("{{{{{key}}}}}"), value);
            }
            ((*path).to_owned(), text)
        })
        .collect()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let name = args
        .iter()
        .position(|a| a == "--name")
        .and_then(|i| args.get(i + 1))
        .ok_or("usage : cargo xtask new-app --name <nom>")?;
    if !valid_name(name) {
        return Err("--name : minuscules, chiffres et tirets, commence par une lettre".into());
    }
    let dest = Path::new("apps").join(name);
    if dest.exists() {
        return Err(format!("{} existe déjà", dest.display()));
    }
    for (path, text) in render(name) {
        let target = dest.join(&path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&target, text).map_err(|e| format!("{} : {e}", target.display()))?;
    }
    println!("✓ {} créé (statut draft, web + desktop).", dest.display());
    println!(
        "  Suite : brand/names.toml → src/engine.rs + ui.rs → locales/ + content/ → cargo xtask registry && cargo xtask check"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_scaffold_has_no_placeholders_and_parses_as_contract() {
        let files = render("demo-app");
        assert!(files.iter().all(|(_, text)| !text.contains("{{")));
        let contract = files
            .iter()
            .find(|(path, _)| path == "app.toml")
            .map(|(_, text)| text.as_str())
            .unwrap_or_default();
        let parsed = kortex_contract::parse_app(contract).expect("template contract parses");
        assert_eq!(parsed.app.id, "apps.demo-app");
        assert!(kortex_contract::validate_app(&parsed).is_empty());
    }

    #[test]
    fn rejects_unsafe_names() {
        assert!(valid_name("flow"));
        assert!(!valid_name("../x"));
        assert!(!valid_name("Flow"));
        assert!(!valid_name("1flow"));
    }
}
