//! Launchers so installed tools show up like normal apps. Best effort: a failure here never
//! fails an installation (the binary itself is already in place and runnable).
use std::path::{Path, PathBuf};

use crate::home::Home;

fn safe_name(name: &str) -> String {
    let n: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .collect();
    let n = n.trim();
    if n.is_empty() {
        "Kortex".to_owned()
    } else {
        n.to_owned()
    }
}

/// Launcher label: "Kortex <name>", without doubling the prefix of app names ("Kortex Store").
fn label(display: &str) -> String {
    let name = safe_name(display);
    match name.strip_prefix("Kortex ") {
        Some(rest) if !rest.trim().is_empty() => format!("Kortex {}", rest.trim()),
        _ if name == "Kortex" => name,
        _ => format!("Kortex {name}"),
    }
}

/// Paths a launcher for `id` would occupy on this OS. The first one is where it is created; the
/// others are older names (e.g. "Kortex Kortex Store.app") that removal still cleans up.
fn locations(home: &Home, id: &str, display: &str) -> Vec<PathBuf> {
    let u = home.user_home();
    let file = id.replace('.', "-");
    let name = safe_name(display);
    if cfg!(target_os = "macos") {
        let apps = u.join("Applications");
        let mut paths = vec![apps.join(format!("{}.app", label(display)))];
        let legacy = apps.join(format!("Kortex {name}.app"));
        if legacy != paths[0] {
            paths.push(legacy);
        }
        paths
    } else if cfg!(target_os = "windows") {
        let dir = u.join("AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Kortex");
        let short = label(display);
        let short = short.strip_prefix("Kortex ").unwrap_or(&short).to_owned();
        let mut paths = vec![dir.join(format!("{short}.cmd"))];
        if short != name {
            paths.push(dir.join(format!("{name}.cmd")));
        }
        paths
    } else {
        let data = kortex_platform::native::env::var_os("XDG_DATA_HOME")
            .map_or_else(|| u.join(".local/share"), PathBuf::from);
        vec![
            data.join("applications")
                .join(format!("kortex-{file}.desktop")),
        ]
    }
}

pub fn create(home: &Home, id: &str, display: &str, binary: &Path) {
    // Launchers left under an older name would otherwise appear twice.
    for old in locations(home, id, display).into_iter().skip(1) {
        let _ = if kortex_platform::native::fs::is_dir(&old) {
            kortex_platform::native::fs::remove_dir_all(&old)
        } else {
            kortex_platform::native::fs::remove_file(&old)
        };
    }
    let _ = try_create(home, id, display, binary);
}

fn try_create(home: &Home, id: &str, display: &str, binary: &Path) -> std::io::Result<()> {
    let name = label(display);
    let target = locations(home, id, display).remove(0);
    let exe = binary.to_string_lossy();
    if cfg!(target_os = "macos") {
        let macos = target.join("Contents/MacOS");
        kortex_platform::native::fs::create_dir_all(&macos)?;
        kortex_platform::native::fs::write(
            target.join("Contents/Info.plist"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleName</key><string>{name}</string><key>CFBundleIdentifier</key><string>site.kortex.{}</string><key>CFBundleExecutable</key><string>launcher</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>\n",
                id.replace('.', "-")
            ),
        )?;
        let launcher = macos.join("launcher");
        kortex_platform::native::fs::write(
            &launcher,
            format!("#!/bin/sh\nexec \"{exe}\" \"$@\"\n"),
        )?;
        set_exec(&launcher)?;
    } else if cfg!(target_os = "windows") {
        if let Some(dir) = target.parent() {
            kortex_platform::native::fs::create_dir_all(dir)?;
        }
        kortex_platform::native::fs::write(
            &target,
            format!("@echo off\r\nstart \"\" \"{exe}\"\r\n"),
        )?;
    } else {
        if let Some(dir) = target.parent() {
            kortex_platform::native::fs::create_dir_all(dir)?;
        }
        kortex_platform::native::fs::write(
            &target,
            format!(
                "[Desktop Entry]\nType=Application\nName={name}\nExec=\"{exe}\"\nTerminal=false\nCategories=Utility;\n"
            ),
        )?;
    }
    Ok(())
}

pub fn remove(home: &Home, id: &str, display: &str) {
    for p in locations(home, id, display) {
        let _ = if kortex_platform::native::fs::is_dir(&p) {
            kortex_platform::native::fs::remove_dir_all(&p)
        } else {
            kortex_platform::native::fs::remove_file(&p)
        };
    }
}

#[must_use]
pub fn existing(home: &Home, id: &str, display: &str) -> Vec<PathBuf> {
    locations(home, id, display)
        .into_iter()
        .filter(|p| kortex_platform::native::fs::exists(p))
        .collect()
}

pub(crate) fn set_exec(p: &Path) -> std::io::Result<()> {
    kortex_platform::native::fs::set_executable(p)
}
