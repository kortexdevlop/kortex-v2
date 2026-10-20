//! Installed tools, reached through their own binary in `--engine` mode (JSON-RPC lines on stdio):
//! `contract` describes the tool, `run` executes it. The CLI links no tool (ADR 0034), so it only
//! changes when its own code does and sees every tool installed on this machine.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use kortex_contract::ToolContract;
use kortex_pkg::PkgEnv;
use kortex_runtime::ValueMap;

const CONTRACT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_RUN_TIMEOUT: Duration = Duration::from_secs(120);

/// Contract of an installed tool, asked to its binary.
///
/// # Errors
/// The binary cannot start or does not answer with a valid contract.
pub fn contract_of(binary: &Path) -> Result<ToolContract, String> {
    let request = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"contract\"}\n";
    let reply = kortex_platform::native::process::exchange(
        binary,
        &["--engine"],
        request,
        CONTRACT_TIMEOUT,
    )
    .map_err(|e| format!("{} ne répond pas : {e}", binary.display()))?;
    let value: serde_json::Value = reply
        .lines()
        .find(|l| !l.trim().is_empty())
        .and_then(|l| serde_json::from_str(l).ok())
        .ok_or("réponse illisible")?;
    let text = value["result"]["contract"]
        .as_str()
        .ok_or("le programme n'a pas décrit son contrat")?;
    kortex_contract::parse(text).map_err(|e| format!("contrat invalide : {e}"))
}

/// Binary of an installed tool, or the actionable error.
///
/// # Errors
/// The tool is not installed on this machine.
pub fn binary_of(env: Option<&PkgEnv>, id: &str) -> Result<PathBuf, String> {
    env.and_then(|e| kortex_pkg::installed_binary(&e.home, id))
        .ok_or_else(|| format!("outil non installé : {id} — lance `kortex install {id}`"))
}

/// Runs an installed tool; `timeout_s` comes from its contract.
///
/// # Errors
/// The tool's own error (`code (detail)`) or a protocol/timeout failure.
pub fn run(
    binary: PathBuf,
    id: &str,
    contract: &ToolContract,
    inputs: ValueMap,
    params: ValueMap,
) -> Result<ValueMap, String> {
    use kortex_workflow::Runner;
    let timeout = contract.limits.timeout_s.map_or(DEFAULT_RUN_TIMEOUT, |s| {
        Duration::from_secs(u64::from(s) + 5)
    });
    let runner = kortex_workflow::process::ProcessRunner {
        binaries: BTreeMap::from([(id.to_owned(), binary)]),
        timeout,
    };
    runner.run(id, inputs, params).map_err(|e| match e.detail {
        Some(d) => format!("{} ({d})", e.code),
        None => e.code,
    })
}

/// Contracts known on this machine: the signed catalogue when reachable, then every installed tool
/// (described by its own binary, which wins). Empty when nothing is installed and offline.
pub fn known(env: Option<&PkgEnv>) -> Vec<ToolContract> {
    let mut all: BTreeMap<String, ToolContract> = BTreeMap::new();
    let Some(env) = env else { return Vec::new() };
    for value in env.load_catalog().unwrap_or_default() {
        if let Ok(c) = serde_json::from_value::<ToolContract>(value) {
            all.insert(c.tool.id.clone(), c);
        }
    }
    if let Ok(registry) = env.home.load_registry() {
        for id in registry.tools.keys() {
            if let Some(binary) = kortex_pkg::installed_binary(&env.home, id)
                && let Ok(c) = contract_of(&binary)
            {
                all.insert(c.tool.id.clone(), c);
            }
        }
    }
    all.into_values().collect()
}
