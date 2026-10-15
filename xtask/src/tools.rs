//! Discovery and loading of tools.
use std::fs;
use std::path::{Path, PathBuf};

use kortex_contract::ToolContract;

pub struct Tool {
    pub dir: PathBuf,
    pub contract: Result<ToolContract, String>,
}

/// Universe ids from `brand/universes.toml`.
pub fn universes() -> Result<Vec<String>, String> {
    let text = fs::read_to_string("brand/universes.toml")
        .map_err(|e| format!("brand/universes.toml : {e}"))?;
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("brand/universes.toml : {e}"))?;
    let list = table
        .get("universe")
        .and_then(|v| v.as_array())
        .ok_or("brand/universes.toml : aucun [[universe]]")?;
    Ok(list
        .iter()
        .filter_map(|u| u.get("id")?.as_str().map(str::to_owned))
        .collect())
}

/// All `tools/<universe>/<name>/` directories (the `_template` is skipped).
pub fn discover() -> Result<Vec<Tool>, String> {
    let mut tools = Vec::new();
    let root = Path::new("tools");
    if !root.exists() {
        return Ok(tools);
    }
    for uni in sorted_dirs(root)? {
        if uni
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('_'))
        {
            continue;
        }
        for dir in sorted_dirs(&uni)? {
            let contract = fs::read_to_string(dir.join("tool.toml"))
                .map_err(|e| format!("tool.toml illisible : {e}"))
                .and_then(|t| kortex_contract::parse(&t));
            tools.push(Tool { dir, contract });
        }
    }
    Ok(tools)
}

fn sorted_dirs(p: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v: Vec<PathBuf> = fs::read_dir(p)
        .map_err(|e| format!("{} : {e}", p.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    v.sort();
    Ok(v)
}
