//! Conservative source guard; native capabilities belong to kortex-platform.
use std::{fs, path::Path};

pub fn check_source(source: &str) -> Result<(), String> {
    let production = source.split("#[cfg(test)]").next().unwrap_or(source);
    let compact = production
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    for forbidden in [
        "std::fs",
        "std::{fs",
        "std::net",
        "std::{net",
        "std::process::Command",
        "std::process::id(",
        "std::env::var",
        "std::env::args",
        "std::env::current",
        "std::io::stdin",
        "std::io::stdout",
        "std::io::stderr",
        "ureq::",
        "rpassword::",
        "web_sys::",
        "js_sys::",
        "getrandom::",
        "document::eval(",
        "SystemTime::now(",
        "std::thread::",
        ".read_bytes(",
    ] {
        if compact.contains(forbidden) {
            return Err(format!(
                "accès plateforme direct `{forbidden}` : utiliser kortex-platform"
            ));
        }
    }
    Ok(())
}
fn visit(path: &Path) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            visit(&path)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let source = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            check_source(&source).map_err(|e| format!("{} : {e}", path.display()))?;
        }
    }
    Ok(())
}
pub fn run() -> Result<(), String> {
    for root in ["crates", "apps", "tools"] {
        for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if !path.is_dir() {
                continue;
            }
            if root == "crates"
                && path
                    .file_name()
                    .is_some_and(|name| name == "kortex-platform" || name == "kortex-macros")
            {
                continue;
            }
            if root == "tools" {
                for tool in fs::read_dir(&path).map_err(|e| e.to_string())? {
                    let src = tool.map_err(|e| e.to_string())?.path().join("src");
                    if src.is_dir() {
                        visit(&src)?;
                    }
                }
            } else {
                let src = path.join("src");
                if src.is_dir() {
                    visit(&src)?;
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_runtime_access_and_allows_platform_and_test_fixtures() {
        for source in [
            "use std::fs as disk;",
            "std::process::Command::new(x)",
            "ureq::get(url)",
            "web_sys::window()",
            "document::eval(script)",
            "js_sys::Date::now()",
            "getrandom::fill(bytes)",
            "file.read_bytes().await",
            "std::env::var(\"HOME\")",
        ] {
            assert!(check_source(source).is_err(), "{source}");
        }
        assert!(check_source("kortex_platform::native::fs::read(p)").is_ok());
        assert!(check_source("fn run() {}\n#[cfg(test)] mod tests { std::fs::read(p); }").is_ok());
    }
}
