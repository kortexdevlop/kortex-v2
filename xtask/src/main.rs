//! Project tooling: `cargo xtask <command>`.
mod check;
mod new_tool;
mod registry;
mod tokens;
mod tools;

use std::process::ExitCode;

const USAGE: &str = "Commandes : tokens | registry | check [id] | new-tool --universe U --name N";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("tokens") => tokens::run(),
        Some("registry") => registry::run(),
        Some("check") => check::run(args.get(1).map(String::as_str)),
        Some("new-tool") => new_tool::run(&args[1..]),
        Some(other) => Err(format!("commande inconnue : {other}\n{USAGE}")),
        None => Err(format!("usage : cargo xtask <commande>\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}
