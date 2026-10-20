//! Contract compatibility against a baseline revision (MASTER_PROMPT §12.2): a tool id never
//! changes or disappears, versions never go down, and a breaking contract change requires a new
//! major version. The baseline is the generated `registry/contracts.json` of another git revision.
use std::collections::BTreeMap;
use std::process::Command;

use kortex_contract::{PortType, ToolContract};

fn version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

fn ports(contract: &ToolContract) -> BTreeMap<(&'static str, &str), (&PortType, bool)> {
    let mut map = BTreeMap::new();
    for input in &contract.inputs {
        map.insert(("entrée", input.id.as_str()), (&input.ty, input.required));
    }
    for param in &contract.params {
        map.insert(("paramètre", param.id.as_str()), (&param.ty, false));
    }
    for output in &contract.outputs {
        map.insert(("sortie", output.id.as_str()), (&output.ty, false));
    }
    map
}

/// Problems of `current` relative to `baseline` (same tool id).
#[must_use]
pub fn compare(baseline: &ToolContract, current: &ToolContract) -> Vec<String> {
    let id = &current.tool.id;
    let mut issues = Vec::new();
    let (Some(old), Some(new)) = (
        version(&baseline.tool.version),
        version(&current.tool.version),
    ) else {
        return vec![format!(
            "{id} : version illisible ({} → {})",
            baseline.tool.version, current.tool.version
        )];
    };
    if new < old {
        issues.push(format!(
            "{id} : la version baisse ({} → {})",
            baseline.tool.version, current.tool.version
        ));
    }
    let (before, after) = (ports(baseline), ports(current));
    let mut breaking = Vec::new();
    for (key, (ty, _)) in &before {
        match after.get(key) {
            None => breaking.push(format!("{} `{}` supprimé(e)", key.0, key.1)),
            Some((new_ty, _)) if new_ty != ty => {
                breaking.push(format!("{} `{}` : type {ty} → {new_ty}", key.0, key.1))
            }
            _ => {}
        }
    }
    for (key, (_, required)) in &after {
        if key.0 == "entrée" && *required && !before.contains_key(key) {
            breaking.push(format!("nouvelle entrée obligatoire `{}`", key.1));
        }
    }
    if !breaking.is_empty() && new.0 == old.0 {
        issues.push(format!(
            "{id} : rupture de contrat sans version majeure ({}) — {}",
            current.tool.version,
            breaking.join(" ; ")
        ));
    }
    issues
}

/// Compares every current contract with the baseline registry text.
///
/// # Errors
/// The baseline JSON cannot be read as a registry.
pub fn compare_registry(
    baseline_json: &str,
    current: &[ToolContract],
) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(baseline_json).map_err(|e| e.to_string())?;
    let tools = value["tools"]
        .as_array()
        .ok_or("registre de référence sans `tools`")?;
    let mut baseline = BTreeMap::new();
    for tool in tools {
        let contract: ToolContract = serde_json::from_value(tool.clone())
            .map_err(|e| format!("contrat de référence illisible : {e}"))?;
        baseline.insert(contract.tool.id.clone(), contract);
    }
    let mut issues = Vec::new();
    let now: BTreeMap<_, _> = current.iter().map(|c| (c.tool.id.as_str(), c)).collect();
    for (id, old) in &baseline {
        match now.get(id.as_str()) {
            Some(new) => issues.extend(compare(old, new)),
            None => issues.push(format!(
                "{id} : présent dans la référence mais absent (un identifiant ne disparaît ni ne change)"
            )),
        }
    }
    Ok(issues)
}

/// Baseline registry text from git: `KORTEX_BASELINE_REF`, else `origin/main` when it exists.
/// `None` when no baseline is available (shallow clone, no remote): the check is then skipped.
#[must_use]
pub fn baseline_registry() -> Option<(String, String)> {
    let reference = std::env::var("KORTEX_BASELINE_REF").unwrap_or_else(|_| "origin/main".into());
    // Revisions before ADR 0034 named the file `registry/index.json`.
    ["registry/contracts.json", "registry/index.json"]
        .iter()
        .find_map(|path| {
            let output = Command::new("git")
                .args(["show", &format!("{reference}:{path}")])
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
        })
        .map(|text| (reference, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(version: &str, extra: &str) -> ToolContract {
        kortex_contract::parse(&format!(
            r#"
schema = 1
[tool]
id = "text.demo"
version = "{version}"
universe = "text"
status = "draft"
ui = "auto"
platforms = ["web"]
name = {{ fr = "D", en = "D" }}
tagline = {{ fr = "x", en = "x" }}
slug = {{ fr = "d", en = "d" }}
[[inputs]]
id = "text"
type = "text"
required = true
[[outputs]]
id = "out"
type = "text"
{extra}
[permissions]
network = false
filesystem = "none"
clipboard = "none"
privacy = "local"
"#
        ))
        .unwrap_or_else(|e| unreachable!("{e}"))
    }

    #[test]
    fn identical_and_additive_changes_pass() {
        let old = contract("1.0.0", "");
        assert!(compare(&old, &contract("1.0.1", "")).is_empty());
        let added = "[[params]]\nid = \"size\"\ntype = \"integer\"\nlevel = \"pro\"\ndefault = 1\n";
        assert!(compare(&old, &contract("1.1.0", added)).is_empty());
    }

    #[test]
    fn version_must_not_go_down() {
        let issues = compare(&contract("1.2.0", ""), &contract("1.1.9", ""));
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("baisse"));
    }

    #[test]
    fn breaking_change_needs_a_major_bump() {
        let with_param =
            "[[params]]\nid = \"size\"\ntype = \"integer\"\nlevel = \"pro\"\ndefault = 1\n";
        let old = contract("1.0.0", with_param);
        let issues = compare(&old, &contract("1.1.0", ""));
        assert!(
            issues
                .iter()
                .any(|i| i.contains("rupture") && i.contains("size")),
            "{issues:?}"
        );
        assert!(compare(&old, &contract("2.0.0", "")).is_empty());
    }

    #[test]
    fn missing_tools_and_unreadable_baselines_are_reported() {
        let issues = compare_registry(r#"{"tools":[]}"#, &[]).unwrap_or_default();
        assert!(issues.is_empty());
        assert!(compare_registry("{}", &[]).is_err());
        let baseline = serde_json::json!({"tools": [serde_json::to_value(contract("1.0.0", "")).unwrap_or_default()]});
        let issues = compare_registry(&baseline.to_string(), &[]).unwrap_or_default();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("absent"));
    }

    #[test]
    fn roundtrips_the_real_generated_registry_as_baseline() {
        let json = std::fs::read_to_string("../registry/contracts.json").unwrap_or_default();
        if json.is_empty() {
            return;
        }
        let current: Vec<ToolContract> = serde_json::from_str::<serde_json::Value>(&json)
            .ok()
            .and_then(|v| v["tools"].as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|t| serde_json::from_value(t).ok())
            .collect();
        assert!(!current.is_empty());
        assert_eq!(
            compare_registry(&json, &current).unwrap_or_default(),
            Vec::<String>::new()
        );
    }
}
