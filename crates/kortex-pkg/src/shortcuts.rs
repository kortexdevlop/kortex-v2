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

/// Paths a launcher for `id` would occupy on this OS.
fn locations(home: &Home, id: &str, display: &str) -> Vec<PathBuf> {
    let u = home.user_home();
    let file = id.replace('.', "-");
    let name = safe_name(display);
    if cfg!(target_os = "macos") {
        vec![u.join("Applications").join(format!("Kortex {name}.app"))]
    } else if cfg!(target_os = "windows") {
        vec![
            u.join("AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Kortex")
                .join(format!("{name}.cmd")),
        ]
    } else {
        let data =
            std::env::var_os("XDG_DATA_HOME").map_or_else(|| u.join(".local/share"), PathBuf::from);
        vec![
            data.join("applications")
                .join(format!("kortex-{file}.desktop")),
        ]
    }
}

pub fn create(home: &Home, id: &str, display: &str, binary: &Path) {
    let _ = try_create(home, id, display, binary);
}

fn try_create(home: &Home, id: &str, display: &str, binary: &Path) -> std::io::Result<()> {
    let name = safe_name(display);
    let target = locations(home, id, display).remove(0);
    let exe = binary.to_string_lossy();
    if cfg!(target_os = "macos") {
        let macos = target.join("Contents/MacOS");
        std::fs::create_dir_all(&macos)?;
        std::fs::write(
            target.join("Contents/Info.plist"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleName</key><string>Kortex {name}</string><key>CFBundleIdentifier</key><string>site.kortex.{}</string><key>CFBundleExecutable</key><string>launcher</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>\n",
                id.replace('.', "-")
            ),
        )?;
        let launcher = macos.join("launcher");
        std::fs::write(&launcher, format!("#!/bin/sh\nexec \"{exe}\" \"$@\"\n"))?;
        set_exec(&launcher)?;
    } else if cfg!(target_os = "windows") {
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&target, format!("@echo off\r\nstart \"\" \"{exe}\"\r\n"))?;
    } else {
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(
            &target,
            format!(
                "[Desktop Entry]\nType=Application\nName=Kortex {name}\nExec=\"{exe}\"\nTerminal=false\nCategories=Utility;\n"
            ),
        )?;
    }
    Ok(())
}

pub fn remove(home: &Home, id: &str, display: &str) {
    for p in locations(home, id, display) {
        let _ = if p.is_dir() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
    }
}

#[must_use]
pub fn existing(home: &Home, id: &str, display: &str) -> Vec<PathBuf> {
    locations(home, id, display)
        .into_iter()
        .filter(|p| p.exists())
        .collect()
}

#[cfg(unix)]
pub(crate) fn set_exec(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
pub(crate) fn set_exec(_: &Path) -> std::io::Result<()> {
    Ok(())
}
