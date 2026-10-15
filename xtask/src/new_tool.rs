//! `cargo xtask new-tool --universe pdf --name merge`
use std::fs;
use std::path::Path;

use crate::tools;

fn copy_dir(from: &Path, to: &Path, vars: &[(&str, String)]) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(from)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
    {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst, vars)?;
        } else if let Ok(mut text) = fs::read_to_string(&src) {
            for (k, v) in vars {
                text = text.replace(&format!("{{{{{k}}}}}"), v);
            }
            fs::write(&dst, text).map_err(|e| e.to_string())?;
        } else {
            fs::copy(&src, &dst).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let universe = arg(args, "--universe")
        .ok_or("usage : cargo xtask new-tool --universe <univers> --name <nom>")?;
    let name = arg(args, "--name").ok_or("--name manquant")?;
    if !tools::universes()?.contains(&universe) {
        return Err(format!(
            "univers `{universe}` inconnu (voir brand/universes.toml)"
        ));
    }
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("--name : minuscules, chiffres et tirets uniquement".into());
    }
    let dest = Path::new("tools").join(&universe).join(&name);
    if dest.exists() {
        return Err(format!("{} existe déjà", dest.display()));
    }
    let pretty = {
        let mut cs = name.replace('-', " ").chars().collect::<Vec<_>>();
        if let Some(f) = cs.first_mut() {
            *f = f.to_ascii_uppercase();
        }
        cs.into_iter().collect::<String>()
    };
    let vars = [
        ("id", format!("{universe}.{name}")),
        ("universe", universe.clone()),
        ("name", name.clone()),
        ("Name", pretty),
        (
            "crate",
            format!("tool_{universe}_{}", name.replace('-', "_")),
        ),
    ];
    copy_dir(Path::new("tools/_template"), &dest, &vars)?;
    println!(
        "✓ {} créé. Étapes : tool.toml → src/engine.rs → glyph.svg → locales/ + content/ → tests/",
        dest.display()
    );
    println!(
        "  Ajoute `tools/{universe}/{name}` aux membres du workspace si besoin, puis : cargo xtask registry && cargo xtask cli-gen && cargo xtask check {universe}.{name}"
    );
    Ok(())
}
