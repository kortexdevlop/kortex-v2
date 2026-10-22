//! Desktop binaries from the shared player (ADR 0035): `kortex-player` is compiled once per
//! target, each tool's payload is written into its reserved region, and on macOS the result is
//! re-signed ad hoc (the file is still a valid executable, so `codesign` accepts it).
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use kortex_pkg::{binary_name, current_target};

use crate::release::{abs, desktop_target_dir};
use crate::{engine, tools};

/// Files of a tool's payload, read from its directory and its built engine.
fn payload(root: &Path, tool_dir: &Path, id: &str) -> Result<Vec<u8>, String> {
    let read = |path: PathBuf| fs::read(&path).map_err(|e| format!("{} : {e}", path.display()));
    let files = vec![
        ("tool.toml".to_owned(), read(tool_dir.join("tool.toml"))?),
        (
            "locales/fr.toml".to_owned(),
            read(tool_dir.join("locales/fr.toml"))?,
        ),
        (
            "locales/en.toml".to_owned(),
            read(tool_dir.join("locales/en.toml"))?,
        ),
        (
            "glyph.svg".to_owned(),
            fs::read(tool_dir.join("glyph.svg")).unwrap_or_default(),
        ),
        (
            "engine.wasm".to_owned(),
            read(root.join("dist/engines").join(id).join("engine.wasm"))?,
        ),
    ];
    kortex_payload::encode(&files).map_err(|e| format!("{id} : {e}"))
}

/// Compiles the desktop player for `target` and returns its path in `dist/players/<target>/`.
pub fn build_player(root: &Path, target: &str) -> Result<PathBuf, String> {
    if !root.join("design/dist/kortex.css").is_file() {
        return Err("design/dist/kortex.css absent : lance `cargo xtask tokens`".to_owned());
    }
    println!("… compilation du lecteur desktop pour {target}");
    let target_dir = desktop_target_dir(root);
    let mut cmd = Command::new("cargo");
    // The `player` profile (root Cargo.toml) builds the dependencies, wasmtime above all, for speed.
    cmd.args(["build", "--profile", "player", "-p", "kortex-player"])
        .current_dir(root)
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("CARGO_PROFILE_PLAYER_STRIP", "symbols");
    let native = Some(target) == current_target();
    if !native {
        cmd.args(["--target", target]);
    }
    if !cmd.status().map_err(|e| e.to_string())?.success() {
        return Err("la compilation du lecteur desktop a échoué".to_owned());
    }
    let exe = if target.contains("windows") {
        "kortex-player.exe"
    } else {
        "kortex-player"
    };
    let built = if native {
        target_dir.join("player").join(exe)
    } else {
        target_dir.join(target).join("player").join(exe)
    };
    let dest = root.join("dist/players").join(target);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let out = dest.join(exe);
    fs::copy(&built, &out).map_err(|e| format!("{} : {e}", built.display()))?;
    Ok(out)
}

fn sign_ad_hoc(binary: &Path) -> Result<(), String> {
    let status = Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(binary)
        .status()
        .map_err(|e| format!("codesign : {e}"))?;
    if !status.success() {
        return Err(format!("signature ad hoc refusée : {}", binary.display()));
    }
    let verified = Command::new("codesign")
        .args(["--verify", "--strict"])
        .arg(binary)
        .status()
        .map_err(|e| format!("codesign : {e}"))?;
    if verified.success() {
        Ok(())
    } else {
        Err(format!(
            "signature ad hoc invalide après assemblage : {}",
            binary.display()
        ))
    }
}

/// Builds the engines, the player, then one assembled binary per id in
/// `dist/desktop/<target>/<id>/`.
pub fn build_desktop(ids: &[String], target: &str) -> Result<(), String> {
    let root = abs(".")?;
    engine::build_engine(ids)?;
    let player_path = build_player(&root, target)?;
    let player = fs::read(&player_path).map_err(|e| e.to_string())?;
    for id in ids {
        let tool = tools::discover()?
            .into_iter()
            .find(|t| t.contract.as_ref().is_ok_and(|c| c.tool.id == *id))
            .ok_or_else(|| format!("outil `{id}` introuvable"))?;
        let payload = payload(&root, &root.join(&tool.dir), id)?;
        let mut image = player.clone();
        kortex_payload::region::fill(&mut image, &payload).map_err(|e| format!("{id} : {e}"))?;
        let dest = root.join("dist/desktop").join(target).join(id);
        fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        let binary = dest.join(binary_name(id, target));
        fs::write(&binary, &image).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        if target.contains("apple-darwin") && cfg!(target_os = "macos") {
            sign_ad_hoc(&binary)?;
        }
        println!(
            "✓ dist/desktop/{target}/{id}/{} ({} Ko, paquet de {} Ko)",
            binary_name(id, target),
            image.len() / 1024,
            payload.len() / 1024
        );
    }
    Ok(())
}
