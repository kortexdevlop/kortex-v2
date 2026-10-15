//! Project tooling: `cargo xtask <command>`.
mod tokens;

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("tokens") => tokens::run(),
        Some(other) => Err(format!("commande inconnue : {other}\nCommandes : tokens")),
        None => Err("usage : cargo xtask <commande>\nCommandes : tokens".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("✗ {msg}");
            ExitCode::FAILURE
        }
    }
}
