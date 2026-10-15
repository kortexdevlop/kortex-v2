//! `cargo xtask stats`: tools per status and universe.
use std::collections::BTreeMap;

use crate::tools;

pub fn run() -> Result<(), String> {
    let mut by_status: BTreeMap<String, u32> = BTreeMap::new();
    let mut by_universe: BTreeMap<String, u32> = BTreeMap::new();
    for t in tools::discover()? {
        let c = t
            .contract
            .map_err(|e| format!("{} : {e}", t.dir.display()))?;
        *by_status
            .entry(format!("{:?}", c.tool.status).to_lowercase())
            .or_default() += 1;
        *by_universe.entry(c.tool.universe).or_default() += 1;
    }
    let total: u32 = by_status.values().sum();
    let join = |m: &BTreeMap<String, u32>| {
        m.iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    println!(
        "Outils : {total}\nPar statut : {}\nPar univers : {}",
        join(&by_status),
        join(&by_universe)
    );
    Ok(())
}
