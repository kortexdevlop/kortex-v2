//! The `kortex` command line: installs, updates and runs tools headlessly through their contract.
//! No tool is linked into it: installed tools are reached through their own binary (`--engine`),
//! so a new tool needs no new CLI (ADR 0034).
use std::io::Write;
use std::path::{Path, PathBuf};

use kortex_contract::{PortType, ToolContract};
use kortex_runtime::{FileData, Value, ValueMap};

pub mod engines;
pub mod pkgcmd;

const USAGE: &str = "kortex — outils locaux en ligne de commande

Usage :
  kortex list [--lang fr|en]
  kortex search <mots…> [--lang fr|en]
  kortex info <outil>
  kortex run <outil> [--input nom=valeur|nom=@chemin]… [--param nom=valeur]… [--out dossier] [--json]
  kortex available | installed | doctor
  kortex install <outil>… | --all        télécharge, vérifie la signature et installe
  kortex update [outil|--all|--self]      met à jour (outils ou la CLI elle-même)
  kortex remove <outil> | open <outil>
  kortex login [--email E] [--password-stdin] | logout | whoami   compte (facultatif)
  kortex version

Exemples :
  kortex run secure.hash-calculator --input text=abc --param algorithm=md5
  kortex run pdf.merge --input files=@a.pdf --input files=@b.pdf --out ./sortie
  kortex run image.convert --input images=@photo.png --param format=jpg";

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("pdf") => "application/pdf",
        Some("txt" | "md") => "text/plain",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn read_file(spec: &str) -> Result<FileData, String> {
    let path = PathBuf::from(spec);
    let bytes = kortex_platform::native::fs::read(&path)
        .map_err(|e| format!("lecture de {spec} impossible : {e}"))?;
    let name = path
        .file_name()
        .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
    Ok(FileData {
        name,
        mime: mime_for(&path).to_owned(),
        bytes,
    })
}

/// `name=value` pairs split on the first `=`.
fn split_kv(s: &str) -> Result<(&str, &str), String> {
    s.split_once('=')
        .ok_or_else(|| format!("`{s}` : attendu nom=valeur"))
}

fn param_value(ty: &PortType, id: &str, raw: &str) -> Result<Value, String> {
    let bad = |what: &str| format!("paramètre {id} : `{raw}` n'est pas {what}");
    Ok(match ty {
        PortType::Integer => Value::Int(raw.trim().parse().map_err(|_| bad("un entier"))?),
        PortType::Number | PortType::Duration | PortType::Range => {
            Value::Number(raw.trim().parse().map_err(|_| bad("un nombre"))?)
        }
        PortType::Bool => Value::Bool(match raw {
            "true" | "1" | "yes" | "oui" => true,
            "false" | "0" | "no" | "non" => false,
            _ => return Err(bad("un booléen (true/false)")),
        }),
        _ => Value::Text(raw.to_owned()),
    })
}

/// Builds the typed input and param maps from `--input` / `--param` arguments.
fn build_maps(
    c: &ToolContract,
    inputs: &[String],
    params: &[String],
) -> Result<(ValueMap, ValueMap), String> {
    let mut in_map = ValueMap::new();
    for arg in inputs {
        let (name, raw) = split_kv(arg)?;
        let port = c
            .inputs
            .iter()
            .find(|i| i.id == name)
            .ok_or_else(|| format!("entrée inconnue : {name}"))?;
        match &port.ty {
            PortType::File(_) => {
                in_map.insert(
                    name.to_owned(),
                    Value::File(read_file(raw.strip_prefix('@').unwrap_or(raw))?),
                );
            }
            PortType::Files(_) => {
                let f = read_file(raw.strip_prefix('@').unwrap_or(raw))?;
                match in_map
                    .entry(name.to_owned())
                    .or_insert_with(|| Value::Files(Vec::new()))
                {
                    Value::Files(v) => v.push(f),
                    _ => return Err(format!("entrée {name} incohérente")),
                }
            }
            PortType::Text | PortType::RichText | PortType::Url | PortType::Secret => {
                let text = match raw.strip_prefix('@') {
                    Some(path) => kortex_platform::native::fs::read_to_string(path)
                        .map_err(|e| format!("lecture de {path} impossible : {e}"))?,
                    None => raw.to_owned(),
                };
                in_map.insert(name.to_owned(), Value::Text(text));
            }
            other => {
                return Err(format!(
                    "entrée {name} : le type {other} n'est pas encore pris en charge en ligne de commande"
                ));
            }
        }
    }
    let mut p_map = ValueMap::new();
    for arg in params {
        let (name, raw) = split_kv(arg)?;
        let p = c
            .params
            .iter()
            .find(|p| p.id == name)
            .ok_or_else(|| format!("paramètre inconnu : {name}"))?;
        p_map.insert(name.to_owned(), param_value(&p.ty, name, raw)?);
    }
    Ok((in_map, p_map))
}

fn lang_of(args: &mut Vec<String>) -> String {
    if let Some(i) = args.iter().position(|a| a == "--lang")
        && i + 1 < args.len()
    {
        let l = args.remove(i + 1);
        args.remove(i);
        return l;
    }
    kortex_platform::native::env::var("LANG")
        .ok()
        .filter(|l| l.starts_with("fr"))
        .map_or("en", |_| "fr")
        .to_owned()
}

fn take_all(args: &mut Vec<String>, flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(i) = args.iter().position(|a| a == flag) {
        if i + 1 >= args.len() {
            break;
        }
        out.push(args.remove(i + 1));
        args.remove(i);
    }
    out
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    args.iter()
        .position(|a| a == flag)
        .map(|i| args.remove(i))
        .is_some()
}

fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' => 'a',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'ù' | 'û' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

fn search(contracts: Vec<ToolContract>, words: &[String], lang: &str) -> Vec<(f64, ToolContract)> {
    let q: Vec<String> = words.iter().map(|w| fold(w)).collect();
    let mut hits = Vec::new();
    for c in contracts {
        let mut hay = vec![
            (c.tool.name.get(lang).cloned().unwrap_or_default(), 3.0),
            (c.tool.id.clone(), 2.0),
            (c.tool.tagline.get(lang).cloned().unwrap_or_default(), 1.0),
        ];
        for k in c.search.keywords.get(lang).into_iter().flatten() {
            hay.push((k.clone(), 2.5));
        }
        for k in c.search.intents.get(lang).into_iter().flatten() {
            hay.push((k.clone(), 2.0));
        }
        let score: f64 = q
            .iter()
            .map(|w| {
                hay.iter()
                    .filter(|(t, _)| fold(t).contains(w.as_str()))
                    .map(|(_, wt)| *wt)
                    .fold(0.0, f64::max)
            })
            .sum();
        if score > 0.0 {
            hits.push((score, c));
        }
    }
    hits.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.tool.id.cmp(&b.1.tool.id)));
    hits
}

fn show_value(v: &Value) -> Option<String> {
    match v {
        Value::Text(t) => Some(t.clone()),
        Value::Number(n) => Some(format!("{n}")),
        Value::Int(i) => Some(i.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Table(t) => Some(
            std::iter::once(t.columns.join(","))
                .chain(t.rows.iter().map(|r| r.join(",")))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Value::File(_) | Value::Files(_) => None,
    }
}

fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Writes output files into `dir` (never overwriting: `name (2).ext`) and returns their paths.
fn save(dir: &Path, f: &FileData) -> Result<PathBuf, String> {
    kortex_platform::native::save_output(dir, &f.name, &f.bytes)
        .map_err(|e| format!("écriture dans {} impossible : {e}", dir.display()))
}

/// Runs the CLI. `args` excludes the program name. Output goes to `out`; errors are returned.
pub fn execute(args: &[String], out: &mut dyn Write) -> Result<(), String> {
    // Package commands need the home directory; engine commands (`run`, `list`…) never do.
    let env = pkgcmd::PkgEnv::from_process();
    execute_with(args, out, env.as_ref().ok())
}

/// Same as [`execute`] with explicit package settings (tests, alternative homes).
pub fn execute_with(
    args: &[String],
    out: &mut dyn Write,
    pkg: Option<&pkgcmd::PkgEnv>,
) -> Result<(), String> {
    execute_io(args, out, pkg, &mut pkgcmd::StdIo)
}

/// Full entry point with an injectable terminal (tests script prompts and use a cheap KDF).
pub fn execute_io(
    args: &[String],
    out: &mut dyn Write,
    pkg: Option<&pkgcmd::PkgEnv>,
    io: &mut dyn pkgcmd::CliIo,
) -> Result<(), String> {
    let mut args = args.to_vec();
    let w = |e: std::io::Error| e.to_string();
    let Some(cmd) = (!args.is_empty()).then(|| args.remove(0)) else {
        writeln!(out, "{USAGE}").map_err(w)?;
        return Ok(());
    };
    match cmd.as_str() {
        "help" | "--help" | "-h" => writeln!(out, "{USAGE}").map_err(w),
        "version" | "--version" | "-V" => {
            writeln!(out, "kortex {}", env!("CARGO_PKG_VERSION")).map_err(w)
        }
        "list" => {
            let lang = lang_of(&mut args);
            for c in engines::known(pkg) {
                writeln!(
                    out,
                    "{}\t{}\t{}",
                    c.tool.id,
                    c.tool.name.get(&lang).cloned().unwrap_or_default(),
                    c.tool.tagline.get(&lang).cloned().unwrap_or_default()
                )
                .map_err(w)?;
            }
            Ok(())
        }
        "search" => {
            let lang = lang_of(&mut args);
            if args.is_empty() {
                return Err("usage : kortex search <mots…>".into());
            }
            for (_, c) in search(engines::known(pkg), &args, &lang) {
                writeln!(
                    out,
                    "{}\t{}",
                    c.tool.id,
                    c.tool.name.get(&lang).cloned().unwrap_or_default()
                )
                .map_err(w)?;
            }
            Ok(())
        }
        "info" => {
            let id = args.first().ok_or("usage : kortex info <outil>")?;
            let c = engines::known(pkg)
                .into_iter()
                .find(|c| &c.tool.id == id)
                .ok_or_else(|| format!("outil inconnu : {id} (voir `kortex list`)"))?;
            writeln!(
                out,
                "{} v{} — {}",
                c.tool.id,
                c.tool.version,
                c.tool.name.get("en").cloned().unwrap_or_default()
            )
            .map_err(w)?;
            for i in &c.inputs {
                writeln!(
                    out,
                    "  entrée  {:<14} {}{}",
                    i.id,
                    i.ty,
                    if i.required { " (requis)" } else { "" }
                )
                .map_err(w)?;
            }
            for p in &c.params {
                let def = p
                    .default
                    .as_ref()
                    .map_or(String::new(), |d| format!(" = {d}"));
                let opts = p
                    .options
                    .as_ref()
                    .map_or(String::new(), |o| format!(" [{}]", o.join("|")));
                let range = p
                    .range
                    .as_ref()
                    .map_or(String::new(), |r| format!(" {r:?}"));
                writeln!(out, "  param   {:<14} {}{def}{opts}{range}", p.id, p.ty).map_err(w)?;
            }
            for o in &c.outputs {
                writeln!(out, "  sortie  {:<14} {}", o.id, o.ty).map_err(w)?;
            }
            Ok(())
        }
        "run" => {
            let json = take_flag(&mut args, "--json");
            let out_dir = take_all(&mut args, "--out")
                .pop()
                .unwrap_or_else(|| ".".to_owned());
            let inputs = take_all(&mut args, "--input");
            let params = take_all(&mut args, "--param");
            let id = args.first().ok_or("usage : kortex run <outil> …")?.clone();
            if let Some(extra) = args.get(1) {
                return Err(format!("argument inattendu : {extra}"));
            }
            let binary = engines::binary_of(pkg, &id)?;
            let c = engines::contract_of(&binary)?;
            let (in_map, p_map) = build_maps(&c, &inputs, &params)?;
            let result = engines::run(binary, &id, &c, in_map, p_map)?;
            let mut json_fields = Vec::new();
            for o in &c.outputs {
                let Some(v) = result.get(&o.id) else { continue };
                let files: Vec<&FileData> = match v {
                    Value::File(f) => vec![f],
                    Value::Files(fs) => fs.iter().collect(),
                    _ => Vec::new(),
                };
                if let Some(text) = show_value(v) {
                    if json {
                        json_fields.push(format!("\"{}\":\"{}\"", o.id, json_escape(&text)));
                    } else {
                        writeln!(out, "{}: {text}", o.id).map_err(w)?;
                    }
                }
                let mut paths = Vec::new();
                for f in files {
                    let p = save(Path::new(&out_dir), f)?;
                    if !json {
                        writeln!(out, "{}: {}", o.id, p.display()).map_err(w)?;
                    }
                    paths.push(format!("\"{}\"", json_escape(&p.to_string_lossy())));
                }
                if json && !paths.is_empty() {
                    json_fields.push(format!("\"{}\":[{}]", o.id, paths.join(",")));
                }
            }
            if json {
                writeln!(out, "{{{}}}", json_fields.join(",")).map_err(w)?;
            }
            Ok(())
        }
        other => {
            if let Some(env) = pkg {
                let lang = lang_of(&mut args);
                if pkgcmd::run(other, &args, out, env, &lang, io)? {
                    return Ok(());
                }
            }
            Err(format!("commande inconnue : {other}\n\n{USAGE}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_files_cannot_escape_the_output_directory() {
        let dir = std::env::temp_dir().join(format!("kortex-cli-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let evil = FileData {
            name: "../../escape.txt".into(),
            mime: String::new(),
            bytes: b"x".to_vec(),
        };
        let path = save(&dir, &evil).expect("saved");
        assert_eq!(path, dir.join("escape.txt"));
        assert!(!dir.parent().expect("parent").join("escape.txt").exists());
    }

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(json_escape("a\"b\\c\n\u{1}"), "a\\\"b\\\\c\\n\\u0001");
    }
}
