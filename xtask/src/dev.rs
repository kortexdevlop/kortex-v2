//! `cargo xtask dev <id> [--desktop]`: runs one tool or app on a single target.
//! Tools are built (receipt-cached) then served as static files; apps use `dx serve` (web,
//! with reload) or `cargo run` (desktop).
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::{apps, build, release, tools};

fn status(mut cmd: Command, what: &str) -> Result<(), String> {
    if cmd.status().map_err(|e| format!("{what} : {e}"))?.success() {
        Ok(())
    } else {
        Err(format!("{what} a échoué"))
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let id = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .ok_or("usage : cargo xtask dev <univers.nom | apps.nom> [--desktop]")?;
    let desktop = args.iter().any(|a| a == "--desktop");
    if let Some(name) = id.strip_prefix("apps.") {
        if !apps::discover()?.iter().any(|a| a.contract.app.id == *id) {
            return Err(format!("application `{id}` introuvable"));
        }
        let package = format!("kortex-{name}");
        let mut cmd = Command::new(if desktop { "cargo" } else { "dx" });
        if desktop {
            cmd.args(["run", "-p", &package]);
        } else {
            cmd.args([
                "serve",
                "--platform",
                "web",
                "--package",
                &package,
                "--no-default-features",
                "--features",
                "web",
            ]);
        }
        return status(cmd, &format!("dev {id}"));
    }
    let known = tools::discover()?
        .into_iter()
        .any(|t| t.contract.is_ok_and(|c| c.tool.id == *id));
    if !known {
        return Err(format!("outil `{id}` introuvable"));
    }
    if desktop {
        release::build_desktop(std::slice::from_ref(id))?;
        let dir = PathBuf::from("dist/desktop");
        let binary = fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|target| target.path().join(id))
            .filter_map(|tool_dir| fs::read_dir(tool_dir).ok())
            .flat_map(|entries| entries.flatten().map(|e| e.path()))
            .next()
            .ok_or("binaire desktop introuvable après compilation")?;
        return status(Command::new(binary), &format!("exécution de {id}"));
    }
    build::run(std::slice::from_ref(id))?;
    println!(
        "→ http://localhost:8080 (rechargement : relancer la commande après une modification)"
    );
    let mut cmd = Command::new("python3");
    cmd.args(["-m", "http.server", "8080", "--directory"])
        .arg(format!("dist/run/{id}"));
    status(cmd, "serveur statique")
}
