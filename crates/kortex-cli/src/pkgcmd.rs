//! Package-management commands: install, update, remove, installed, doctor, open.
use std::io::Write;

use kortex_pkg::{
    Index, Kind, PkgError, doctor, install, installed_binary, remove, self_update, statuses, update,
};

pub use kortex_pkg::{DEFAULT_INDEX, PkgEnv};

fn w(e: std::io::Error) -> String {
    e.to_string()
}

fn lang_name(names: &std::collections::BTreeMap<String, String>, lang: &str, id: &str) -> String {
    names
        .get(lang)
        .or_else(|| names.get("en"))
        .cloned()
        .unwrap_or_else(|| id.to_owned())
}

fn phase_label(p: &str) -> &str {
    match p {
        "download" => "téléchargement",
        "verify" => "vérification (signature, empreinte, contenu)",
        "install" => "installation",
        _ => "",
    }
}

fn install_one(env: &PkgEnv, index: &Index, id: &str, out: &mut dyn Write) -> Result<(), String> {
    let target = env.target().map_err(|e| e.to_string())?;
    let mut note = |p: &str| {
        if !phase_label(p).is_empty() {
            let _ = writeln!(out, "  {id} : {}…", phase_label(p));
        }
    };
    let t = install(&env.home, index, &env.index_src, id, target, &mut note)
        .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "✓ {id} {} installé : {}",
        t.version,
        env.home.binary_path(&t).display()
    )
    .map_err(w)
}

pub fn run(
    cmd: &str,
    args: &[String],
    out: &mut dyn Write,
    env: &PkgEnv,
    lang: &str,
) -> Result<bool, String> {
    match cmd {
        "install" => {
            if args.is_empty() {
                return Err("usage : kortex install <outil>… | kortex install --all".into());
            }
            let index = env.load_index().map_err(|e| e.to_string())?;
            let ids: Vec<String> = if args.iter().any(|a| a == "--all") {
                index.tools.keys().cloned().collect()
            } else {
                args.to_vec()
            };
            for id in ids {
                install_one(env, &index, &id, out)?;
            }
            Ok(true)
        }
        "update" => {
            let index = env.load_index().map_err(|e| e.to_string())?;
            let target = env.target().map_err(|e| e.to_string())?.to_owned();
            if args.iter().any(|a| a == "--self") {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                match self_update(
                    &index,
                    &env.index_src,
                    &exe,
                    &target,
                    env!("CARGO_PKG_VERSION"),
                )
                .map_err(|e| e.to_string())?
                {
                    Some(v) => writeln!(out, "✓ kortex mis à jour en {v}").map_err(w)?,
                    None => writeln!(out, "kortex est à jour ({})", env!("CARGO_PKG_VERSION"))
                        .map_err(w)?,
                }
                return Ok(true);
            }
            let reg = env.home.load_registry().map_err(|e| e.to_string())?;
            let ids: Vec<String> = if args.is_empty() || args.iter().any(|a| a == "--all") {
                reg.tools.keys().cloned().collect()
            } else {
                args.to_vec()
            };
            if ids.is_empty() {
                writeln!(out, "Aucun outil installé.").map_err(w)?;
            }
            for id in ids {
                let mut note = |_: &str| {};
                match update(&env.home, &index, &env.index_src, &id, &target, &mut note)
                    .map_err(|e| e.to_string())?
                {
                    Some(t) => writeln!(out, "✓ {id} mis à jour en {}", t.version).map_err(w)?,
                    None => writeln!(out, "{id} est à jour").map_err(w)?,
                }
            }
            Ok(true)
        }
        "remove" => {
            let id = args.first().ok_or("usage : kortex remove <outil>")?;
            // The display name only names the launcher; the index may be unreachable offline.
            let display = env
                .load_index()
                .ok()
                .and_then(|i| i.tools.get(id).map(|t| lang_name(&t.name, "en", id)))
                .unwrap_or_else(|| id.clone());
            remove(&env.home, id, &display).map_err(|e| e.to_string())?;
            writeln!(out, "✓ {id} supprimé").map_err(w)?;
            Ok(true)
        }
        "installed" => {
            let reg = env.home.load_registry().map_err(|e| e.to_string())?;
            let index = env.load_index().ok(); // works offline: update info is just omitted
            let st = index
                .as_ref()
                .map(|i| statuses(i, &reg, env.target.as_deref()));
            if reg.tools.is_empty() {
                writeln!(out, "Aucun outil installé.").map_err(w)?;
            }
            for (id, t) in &reg.tools {
                let upd = st
                    .as_ref()
                    .and_then(|s| s.iter().find(|x| x.id == *id))
                    .filter(|x| x.update_available)
                    .map_or(String::new(), |x| {
                        format!("  → mise à jour disponible : {}", x.available)
                    });
                writeln!(out, "{id}\t{}{upd}", t.version).map_err(w)?;
            }
            Ok(true)
        }
        "available" => {
            let index = env.load_index().map_err(|e| e.to_string())?;
            let reg = env.home.load_registry().map_err(|e| e.to_string())?;
            for s in statuses(&index, &reg, env.target.as_deref()) {
                let state = match (&s.installed, s.update_available, s.supported) {
                    (_, _, false) => "indisponible sur cette plateforme".to_owned(),
                    (Some(v), true, _) => format!("installé {v}, mise à jour {}", s.available),
                    (Some(v), false, _) => format!("installé {v}"),
                    (None, _, _) => format!("disponible {}", s.available),
                };
                writeln!(
                    out,
                    "{}\t{}\t{state}",
                    s.id,
                    lang_name(&s.name, lang, &s.id)
                )
                .map_err(w)?;
            }
            Ok(true)
        }
        "doctor" => {
            let mut bad = 0;
            for c in doctor(&env.home) {
                writeln!(
                    out,
                    "{} {} — {}",
                    if c.ok { "✓" } else { "✗" },
                    c.name,
                    c.detail
                )
                .map_err(w)?;
                bad += usize::from(!c.ok);
            }
            match env.load_index() {
                Ok(i) => writeln!(
                    out,
                    "✓ index — signature valide, {} outil(s) disponible(s)",
                    i.tools.len()
                )
                .map_err(w)?,
                Err(e) => {
                    bad += 1;
                    writeln!(out, "✗ index — {e}").map_err(w)?;
                }
            }
            if bad > 0 {
                Err(format!("{bad} problème(s) détecté(s)"))
            } else {
                Ok(true)
            }
        }
        "open" => {
            let id = args.first().ok_or("usage : kortex open <outil>")?;
            let bin = installed_binary(&env.home, id)
                .ok_or_else(|| format!("{id} n'est pas installé (kortex install {id})"))?;
            std::process::Command::new(&bin)
                .spawn()
                .map_err(|e| format!("lancement de {} impossible : {e}", bin.display()))?;
            writeln!(out, "{id} lancé").map_err(w)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Kinds worth a friendlier hint at the CLI.
pub fn hint(e: &PkgError) -> Option<&'static str> {
    match e.kind() {
        Kind::BadSignature => Some("l'index n'est pas signé par Kortex : ne l'utilise pas"),
        Kind::Network => Some("vérifie ta connexion, ou KORTEX_INDEX"),
        _ => None,
    }
}
