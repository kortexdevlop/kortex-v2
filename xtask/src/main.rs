//! Project tooling: `cargo xtask <command>`.
mod build;
mod check;
mod cli_gen;
mod icons;
mod new_tool;
mod registry;
mod release;
mod stats;
mod tokens;
mod tools;

use std::process::ExitCode;

const USAGE: &str = "Commandes : tokens | registry | check [id] | stats | icons | cli-gen | keygen | build-desktop | build-cli | build-app | list-tools | package | release-index | build <id> | new-tool --universe U --name N";

/// Path for embedding in generated TOML/Rust source: forward slashes work on every OS, while
/// Windows backslashes would be read as (invalid) escape sequences.
pub fn fwd(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("tokens") => tokens::run(),
        Some("registry") => registry::run(),
        Some("check") => check::run(args.get(1).map(String::as_str)),
        Some("build") => build::run(&args[1..]),
        Some("keygen") => release::keygen(&args[1..]),
        Some("build-desktop") => release::build_desktop(&args[1..]),
        Some("list-tools") => tools::discover().and_then(|ts| {
            ts.into_iter().try_for_each(|t| {
                t.contract
                    .map(|c| println!("{}", c.tool.id))
                    .map_err(|e| e.to_string())
            })
        }),
        Some("build-app") => release::build_app(&args[1..]),
        Some("build-cli") => release::build_cli(&args[1..]),
        Some("package") => release::package(&args[1..]),
        Some("release-index") => release::release_index(&args[1..]),
        Some("cli-gen") => cli_gen::run(),
        Some("icons") => icons::run(),
        Some("stats") => stats::run(),
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
