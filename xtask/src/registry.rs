//! `registry/index.json`: generated from every `tool.toml`.
use std::fs;

use crate::tools;

/// Build the registry JSON text (deterministic).
pub fn build() -> Result<String, String> {
    let mut list = Vec::new();
    for t in tools::discover()? {
        let c = t
            .contract
            .map_err(|e| format!("{} : {e}", t.dir.display()))?;
        list.push(c);
    }
    list.sort_by(|a, b| a.tool.id.cmp(&b.tool.id));
    let doc = serde_json::json!({ "schema": 1, "tools": list });
    serde_json::to_string_pretty(&doc)
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

pub fn run() -> Result<(), String> {
    let json = build()?;
    fs::create_dir_all("registry").map_err(|e| e.to_string())?;
    fs::write("registry/index.json", &json).map_err(|e| e.to_string())?;
    println!("✓ registry/index.json généré");
    Ok(())
}
