use std::collections::HashSet;

use crate::{LANGS, Localized, ParamLevel, PortType, ToolContract};

/// A contract problem, with an actionable French message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue(pub String);

fn lang_check(issues: &mut Vec<Issue>, what: &str, l: &Localized) {
    for lang in LANGS {
        if l.get(lang).is_none_or(|s| s.trim().is_empty()) {
            issues.push(Issue(format!("{what} : traduction `{lang}` manquante")));
        }
    }
}

fn is_semver(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn is_ident(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next().is_some_and(|c| c.is_ascii_lowercase())
        && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !matches!(
            s,
            "type"
                | "match"
                | "use"
                | "fn"
                | "self"
                | "mod"
                | "in"
                | "as"
                | "ref"
                | "move"
                | "loop"
                | "where"
                | "impl"
                | "trait"
                | "struct"
                | "enum"
                | "for"
                | "if"
                | "else"
                | "let"
                | "mut"
                | "pub"
                | "return"
                | "static"
                | "super"
                | "true"
                | "false"
                | "while"
                | "async"
                | "await"
                | "crate"
                | "const"
                | "dyn"
                | "box"
                | "extern"
                | "break"
                | "continue"
                | "unsafe"
        )
}

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Validate a contract. `universes` is the list of ids from `brand/universes.toml`.
#[must_use]
pub fn validate(c: &ToolContract, universes: &[String]) -> Vec<Issue> {
    validate_contract(c, universes, true)
}

pub(crate) fn validate_contract(
    c: &ToolContract,
    universes: &[String],
    require_outputs: bool,
) -> Vec<Issue> {
    let mut issues = Vec::new();
    let t = &c.tool;
    if c.schema != 1 {
        issues.push(Issue(format!(
            "schema = {} non supporté (attendu : 1)",
            c.schema
        )));
    }
    match t.id.split_once('.') {
        Some((u, n)) if u == t.universe && is_slug(n) => {}
        _ => issues.push(Issue(format!(
            "tool.id `{}` doit valoir `<univers>.<nom>` avec l'univers `{}` (minuscules, chiffres, tirets)",
            t.id, t.universe
        ))),
    }
    if !universes.contains(&t.universe) {
        issues.push(Issue(format!(
            "univers `{}` absent de brand/universes.toml",
            t.universe
        )));
    }
    if !is_semver(&t.version) {
        issues.push(Issue(format!(
            "version `{}` n'est pas du semver X.Y.Z",
            t.version
        )));
    }
    if t.platforms.is_empty() || t.platforms.iter().any(|p| p != "web" && p != "desktop") {
        issues.push(Issue(
            "platforms doit contenir `web` et/ou `desktop`".into(),
        ));
    }
    lang_check(&mut issues, "tool.name", &t.name);
    lang_check(&mut issues, "tool.tagline", &t.tagline);
    lang_check(&mut issues, "tool.slug", &t.slug);
    for (lang, slug) in &t.slug {
        if !is_slug(slug) {
            issues.push(Issue(format!(
                "slug {lang} `{slug}` invalide (a-z, 0-9, -)"
            )));
        }
    }

    for id in c
        .inputs
        .iter()
        .map(|i| &i.id)
        .chain(c.params.iter().map(|p| &p.id))
        .chain(c.outputs.iter().map(|o| &o.id))
    {
        if !is_ident(id) {
            issues.push(Issue(format!(
                "identifiant `{id}` invalide : snake_case ASCII (a-z, 0-9, _), pas un mot-clé Rust"
            )));
        }
    }

    let mut ids = HashSet::new();
    let mut dup = |kind: &str, id: &str, issues: &mut Vec<Issue>| {
        if !ids.insert(format!("{kind}:{id}")) {
            issues.push(Issue(format!("{kind} `{id}` déclaré deux fois")));
        }
    };
    for i in &c.inputs {
        dup("input", &i.id, &mut issues);
    }
    for o in &c.outputs {
        dup("output", &o.id, &mut issues);
        if matches!(o.ty, PortType::File(_) | PortType::Files(_))
            && o.formats.as_ref().is_none_or(Vec::is_empty)
        {
            issues.push(Issue(format!(
                "sortie `{}` : `formats` obligatoire pour un fichier",
                o.id
            )));
        }
        if o.ty == PortType::Secret {
            issues.push(Issue(format!(
                "sortie `{}` : un `secret` ne peut pas être une sortie",
                o.id
            )));
        }
    }
    if require_outputs && c.outputs.is_empty() {
        issues.push(Issue(
            "au moins une sortie `[[outputs]]` est requise".into(),
        ));
    }
    for p in &c.params {
        dup("param", &p.id, &mut issues);
        if p.level.is_none() {
            issues.push(Issue(format!(
                "paramètre `{}` : `level` (simple|medium|pro) manquant",
                p.id
            )));
        }
        if p.ty == PortType::Enum && p.options.as_ref().is_none_or(Vec::is_empty) {
            issues.push(Issue(format!(
                "paramètre `{}` : `options` obligatoire pour un enum",
                p.id
            )));
        }
        if p.default.is_none() && p.level == Some(ParamLevel::Simple) {
            issues.push(Issue(format!(
                "paramètre `{}` : un `default` est requis (mode Simple)",
                p.id
            )));
        }
    }

    let perm = &c.permissions;
    if !["none", "read", "write"].contains(&perm.filesystem.as_str()) {
        issues.push(Issue(
            "permissions.filesystem doit valoir none|read|write".into(),
        ));
    }
    if !["none", "read", "write"].contains(&perm.clipboard.as_str()) {
        issues.push(Issue(
            "permissions.clipboard doit valoir none|read|write".into(),
        ));
    }
    if !["local", "ai-consent"].contains(&perm.privacy.as_str()) {
        issues.push(Issue(
            "permissions.privacy doit valoir local|ai-consent".into(),
        ));
    }
    if t.ai && perm.privacy != "ai-consent" {
        issues.push(Issue(
            "un outil `ai = true` doit déclarer `privacy = \"ai-consent\"`".into(),
        ));
    }
    for v in &c.variants {
        lang_check(&mut issues, &format!("variante `{}` (name)", v.id), &v.name);
        lang_check(&mut issues, &format!("variante `{}` (slug)", v.id), &v.slug);
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = r#"
schema = 1
[tool]
id = "text.case"
version = "1.0.0"
universe = "text"
status = "draft"
ui = "auto"
platforms = ["web", "desktop"]
name = { fr = "Casse", en = "Case" }
tagline = { fr = "x", en = "x" }
slug = { fr = "casse", en = "case" }
[[inputs]]
id = "text"
type = "text"
[[outputs]]
id = "text"
type = "text"
[permissions]
network = false
filesystem = "none"
clipboard = "none"
privacy = "local"
"#;

    fn universes() -> Vec<String> {
        vec!["text".into()]
    }

    #[test]
    fn valid_contract_has_no_issue() {
        let c = crate::parse(OK).expect("parse");
        assert_eq!(validate(&c, &universes()), vec![]);
    }

    #[test]
    fn detects_problems() {
        let bad = OK
            .replace("text.case", "image.case")
            .replace("1.0.0", "1.0");
        let c = crate::parse(&bad).expect("parse");
        let msgs: Vec<String> = validate(&c, &universes())
            .into_iter()
            .map(|i| i.0)
            .collect();
        assert!(msgs.iter().any(|m| m.contains("tool.id")));
        assert!(msgs.iter().any(|m| m.contains("semver")));
    }

    #[test]
    fn unknown_port_type_fails_parse() {
        assert!(
            crate::parse(&OK.replace(
                "type = \"text\"\n[[outputs]]",
                "type = \"nope\"\n[[outputs]]"
            ))
            .is_err()
        );
    }
}
