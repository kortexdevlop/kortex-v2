//! Package and account commands: install, update, remove, installed, available, doctor, open,
//! login, logout, whoami.
use std::io::Write;

use kortex_pkg::{
    Credentials, Index, KDF_ITERATIONS, doctor, install, installed_binary, remove, self_update,
    statuses, update,
};

pub use kortex_pkg::{DEFAULT_API, DEFAULT_INDEX, PkgEnv};

/// Terminal access, injectable so tests can script answers and use a cheap KDF.
pub trait CliIo {
    /// Visible answer (e-mail). `None` = no terminal / cancelled.
    fn prompt(&mut self, label: &str) -> Option<String>;
    /// Hidden answer (password). `from_stdin` reads one line from stdin instead of the terminal.
    fn password(&mut self, label: &str, from_stdin: bool) -> Option<String>;
    fn kdf_iterations(&self) -> u32 {
        KDF_ITERATIONS
    }
}

pub struct StdIo;

impl CliIo for StdIo {
    fn prompt(&mut self, label: &str) -> Option<String> {
        kortex_platform::native::terminal_line(Some(label), true)
    }
    fn password(&mut self, label: &str, from_stdin: bool) -> Option<String> {
        if from_stdin {
            kortex_platform::native::terminal_line(None, false)
        } else {
            kortex_platform::native::terminal_password(label)
        }
    }
}

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

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    args.iter()
        .position(|a| a == flag)
        .map(|i| args.remove(i))
        .is_some()
}

fn take_value(args: &mut Vec<String>, flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    (i + 1 < args.len()).then(|| {
        let v = args.remove(i + 1);
        args.remove(i);
        v
    })
}

fn device_name() -> String {
    kortex_platform::native::device_name()
}

/// If this machine is linked to an account that asks for it, checks the password before changing
/// anything. Fails closed: when the account server cannot be reached, nothing is installed.
fn confirm_with_account(
    creds: Option<&Credentials>,
    io: &mut dyn CliIo,
    from_stdin: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let Some(c) = creds else { return Ok(()) };
    let required = kortex_pkg::confirm_required(c).map_err(|e| format!("impossible de vérifier ton compte ({e}) : réessaie en ligne, ou `kortex logout` pour installer sans compte"))?;
    if !required {
        return Ok(());
    }
    let pw = io.password(&format!("Mot de passe de {} (confirme l'installation) : ", c.email), from_stdin).ok_or("mot de passe requis : lance la commande dans un terminal, utilise --password-stdin, ou désactive la confirmation dans ton compte")?;
    match kortex_pkg::confirm_password(c, &pw, io.kdf_iterations()).map_err(|e| e.to_string())? {
        true => {
            writeln!(out, "✓ identité confirmée").map_err(w)?;
            Ok(())
        }
        false => Err("mot de passe incorrect : installation annulée".into()),
    }
}

fn install_one(
    env: &PkgEnv,
    index: &Index,
    id: &str,
    creds: Option<&Credentials>,
    out: &mut dyn Write,
) -> Result<(), String> {
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
    .map_err(w)?;
    if let Some(c) = creds
        && let Err(e) = kortex_pkg::record_install(c, id, &t.version, target)
    {
        writeln!(out, "  (installation non enregistrée sur ton compte : {e})").map_err(w)?;
    }
    Ok(())
}

pub fn run(
    cmd: &str,
    args: &[String],
    out: &mut dyn Write,
    env: &PkgEnv,
    lang: &str,
    io: &mut dyn CliIo,
) -> Result<bool, String> {
    let mut args = args.to_vec();
    match cmd {
        "login" => {
            let from_stdin = take_flag(&mut args, "--password-stdin");
            let name = take_value(&mut args, "--name").unwrap_or_else(device_name);
            let email = match take_value(&mut args, "--email") {
                Some(e) => e,
                None => io.prompt("E-mail : ").ok_or("e-mail requis (--email)")?,
            };
            let password = io
                .password("Mot de passe : ", from_stdin)
                .ok_or("mot de passe requis (terminal ou --password-stdin)")?;
            let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
            let c = kortex_pkg::login_device(
                &env.home,
                &env.api,
                &email,
                &password,
                &name,
                &platform,
                io.kdf_iterations(),
            )
            .map_err(|e| e.to_string())?;
            writeln!(
                out,
                "✓ connecté en tant que {} (appareil « {name} », révocable depuis ton compte)",
                c.email
            )
            .map_err(w)?;
            Ok(true)
        }
        "logout" => {
            match kortex_pkg::logout(&env.home).map_err(|e| e.to_string())? {
                true => writeln!(out, "✓ déconnecté, appareil révoqué"),
                false => writeln!(out, "Déconnecté localement (le serveur n'a pas pu être prévenu : révoque l'appareil depuis ton compte)."),
            }
            .map_err(w)?;
            Ok(true)
        }
        "whoami" => {
            let Some(c) = env.home.load_credentials() else {
                writeln!(out, "Non connecté (kortex login).").map_err(w)?;
                return Ok(true);
            };
            match kortex_pkg::whoami(&c).map_err(|e| e.to_string())? {
                Some(email) => writeln!(out, "{email} — appareil {}", c.device_id).map_err(w)?,
                None => {
                    writeln!(out, "Jeton révoqué ou expiré : relance `kortex login`.").map_err(w)?
                }
            }
            Ok(true)
        }
        "install" => {
            let from_stdin = take_flag(&mut args, "--password-stdin");
            if args.is_empty() {
                return Err("usage : kortex install <outil>… | kortex install --all".into());
            }
            let index = env.load_index().map_err(|e| e.to_string())?;
            let ids: Vec<String> = if take_flag(&mut args, "--all") {
                index.tools.keys().cloned().collect()
            } else {
                args
            };
            let creds = env.home.load_credentials();
            confirm_with_account(creds.as_ref(), io, from_stdin, out)?;
            for id in ids {
                install_one(env, &index, &id, creds.as_ref(), out)?;
            }
            Ok(true)
        }
        "update" => {
            let index = env.load_index().map_err(|e| e.to_string())?;
            let target = env.target().map_err(|e| e.to_string())?.to_owned();
            if take_flag(&mut args, "--self") {
                let exe = kortex_platform::native::env::current_exe().map_err(|e| e.to_string())?;
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
            let all = take_flag(&mut args, "--all");
            let ids: Vec<String> = if args.is_empty() || all {
                reg.tools.keys().cloned().collect()
            } else {
                args
            };
            if ids.is_empty() {
                writeln!(out, "Aucun outil installé.").map_err(w)?;
            }
            let creds = env.home.load_credentials();
            for id in ids {
                let mut note = |_: &str| {};
                match update(&env.home, &index, &env.index_src, &id, &target, &mut note)
                    .map_err(|e| e.to_string())?
                {
                    Some(t) => {
                        writeln!(out, "✓ {id} mis à jour en {}", t.version).map_err(w)?;
                        if let Some(c) = &creds {
                            let _ = kortex_pkg::record_install(c, &id, &t.version, &target);
                        }
                    }
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
            if let Some(c) = env.home.load_credentials() {
                let _ = kortex_pkg::forget_install(&c, id);
            }
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
            match env.home.load_credentials() {
                Some(c) => match kortex_pkg::whoami(&c) {
                    Ok(Some(email)) => writeln!(out, "✓ compte — {email}").map_err(w)?,
                    Ok(None) => {
                        bad += 1;
                        writeln!(out, "✗ compte — jeton révoqué ou expiré (kortex login)")
                            .map_err(w)?;
                    }
                    Err(e) => writeln!(out, "• compte — non vérifié ({e})").map_err(w)?,
                },
                None => writeln!(out, "• compte — non connecté (facultatif : kortex login)")
                    .map_err(w)?,
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
            kortex_platform::native::launch_executable(&bin)
                .map_err(|e| format!("lancement de {} impossible : {e}", bin.display()))?;
            writeln!(out, "{id} lancé").map_err(w)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
