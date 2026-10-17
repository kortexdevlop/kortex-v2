//! `cargo xtask schema <id|all> [--lang fr|en]`: JSON Schema tool definitions from contracts.
use crate::tools;

pub fn run(args: &[String]) -> Result<(), String> {
    let target = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .ok_or("usage : cargo xtask schema <univers.nom | all> [--lang fr|en]")?;
    let lang = args
        .iter()
        .position(|a| a == "--lang")
        .and_then(|i| args.get(i + 1))
        .map_or("en", String::as_str);
    if !kortex_contract::LANGS.contains(&lang) {
        return Err(format!("--lang : `{lang}` inconnue (fr ou en)"));
    }
    let mut definitions = Vec::new();
    for tool in tools::discover()? {
        let contract = tool
            .contract
            .map_err(|e| format!("{} : {e}", tool.dir.display()))?;
        if target == "all" || contract.tool.id == *target {
            definitions.push(kortex_contract::tool_definition(&contract, lang));
        }
    }
    if definitions.is_empty() {
        return Err(format!("outil `{target}` introuvable"));
    }
    let output = if target == "all" {
        serde_json::Value::Array(definitions)
    } else {
        definitions.swap_remove(0)
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|e| e.to_string())?
    );
    Ok(())
}
