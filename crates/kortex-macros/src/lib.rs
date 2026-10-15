//! `kortex::contract!("tool.toml")` and `#[kortex::engine]`.
//! No `syn`/`quote`: code is generated as text from the validated contract.
use std::fmt::Write as _;

use kortex_contract::{ParamLevel, PortType, ToolContract};
use proc_macro::TokenStream;

fn compile_error(msg: &str) -> TokenStream {
    format!("compile_error!({msg:?});")
        .parse()
        .unwrap_or_default()
}

fn rust_type(ty: &PortType) -> &'static str {
    match ty {
        PortType::Number | PortType::Duration | PortType::Range => "f64",
        PortType::Integer => "i64",
        PortType::Bool => "bool",
        PortType::Table => "::kortex::runtime::Table",
        PortType::File(_) => "::kortex::runtime::FileData",
        PortType::Files(_) => "::std::vec::Vec<::kortex::runtime::FileData>",
        _ => "::std::string::String",
    }
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

fn default_expr(ty: &PortType, v: Option<&toml::Value>) -> String {
    use toml::Value::{Boolean, Float, Integer, String as S};
    match (ty, v) {
        (PortType::Integer, Some(Integer(i))) => format!("{i}_i64"),
        (PortType::Number | PortType::Duration | PortType::Range, Some(Float(f))) => {
            format!("{f:?}_f64")
        }
        #[allow(clippy::cast_precision_loss)]
        (PortType::Number | PortType::Duration | PortType::Range, Some(Integer(i))) => {
            format!("{:?}_f64", *i as f64)
        }
        (PortType::Bool, Some(Boolean(b))) => b.to_string(),
        (_, Some(S(s))) if rust_type(ty).ends_with("String") => {
            format!("::std::string::String::from({s:?})")
        }
        _ => "::core::default::Default::default()".to_owned(),
    }
}

fn gen_struct(out: &mut String, name: &str, fields: &[(String, String, Option<String>)]) {
    let _ = writeln!(
        out,
        "#[derive(Debug, Clone, PartialEq)]\npub struct {name} {{"
    );
    for (id, ty, _) in fields {
        let _ = writeln!(out, "    pub {id}: {ty},");
    }
    let _ = writeln!(out, "}}");
    let _ = writeln!(
        out,
        "impl ::core::default::Default for {name} {{\n    fn default() -> Self {{\n        Self {{"
    );
    for (id, _, d) in fields {
        let _ = writeln!(
            out,
            "            {id}: {},",
            d.as_deref()
                .unwrap_or("::core::default::Default::default()")
        );
    }
    let _ = writeln!(out, "        }}\n    }}\n}}");
}

fn generate(c: &ToolContract, raw_path: &str) -> Result<String, String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "pub const CONTRACT: &str = include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{raw_path}\"));"
    );
    let _ = writeln!(out, "pub const TOOL_ID: &str = {:?};", c.tool.id);
    let _ = writeln!(out, "pub const TOOL_VERSION: &str = {:?};", c.tool.version);

    for id in c
        .inputs
        .iter()
        .map(|i| &i.id)
        .chain(c.params.iter().map(|p| &p.id))
        .chain(c.outputs.iter().map(|o| &o.id))
    {
        if !is_ident(id) {
            return Err(format!(
                "identifiant `{id}` invalide : snake_case ASCII, pas un mot-clé Rust"
            ));
        }
    }

    let inputs: Vec<_> = c
        .inputs
        .iter()
        .map(|i| {
            let t = rust_type(&i.ty);
            if i.required {
                (i.id.clone(), t.to_owned(), None)
            } else {
                (i.id.clone(), format!("::core::option::Option<{t}>"), None)
            }
        })
        .collect();
    gen_struct(&mut out, "Inputs", &inputs);

    let params: Vec<_> = c
        .params
        .iter()
        .map(|p| {
            (
                p.id.clone(),
                rust_type(&p.ty).to_owned(),
                Some(default_expr(&p.ty, p.default.as_ref())),
            )
        })
        .collect();
    gen_struct(&mut out, "Params", &params);

    let outputs: Vec<_> = c
        .outputs
        .iter()
        .map(|o| (o.id.clone(), rust_type(&o.ty).to_owned(), None))
        .collect();
    gen_struct(&mut out, "Outputs", &outputs);

    // Param validation: ranges.
    let _ = writeln!(
        out,
        "impl Params {{\n    /// Check numeric ranges declared in the contract.\n    ///\n    /// # Errors\n    /// `invalid_param` when a value is outside its range.\n    pub fn validate(&self) -> ::core::result::Result<(), ::kortex::runtime::ToolError> {{"
    );
    for p in &c.params {
        if let Some(r) = &p.range
            && r.len() == 2
            && matches!(p.ty, PortType::Integer | PortType::Number)
        {
            let _ = writeln!(
                out,
                "        if !(({:?}_f64)..=({:?}_f64)).contains(&(self.{} as f64)) {{ return ::core::result::Result::Err(::kortex::runtime::ToolError::with_detail(::kortex::runtime::ToolError::INVALID_PARAM, {:?})); }}",
                r[0], r[1], p.id, p.id
            );
        }
    }
    let _ = writeln!(out, "        ::core::result::Result::Ok(())\n    }}\n}}");

    // Dynamic entry points: ValueMap <-> typed structs, and `run_map` (calls `engine::run`).
    let _ = writeln!(
        out,
        "impl Inputs {{\n    /// # Errors\n    /// `missing_input` / `invalid_param`.\n    pub fn from_map(m: &::kortex::runtime::ValueMap) -> ::core::result::Result<Self, ::kortex::runtime::ToolError> {{\n        ::core::result::Result::Ok(Self {{"
    );
    for i in &c.inputs {
        let t = rust_type(&i.ty);
        if i.required {
            let _ = writeln!(
                out,
                "            {id}: <{t} as ::kortex::runtime::FromValue>::from_value(m.get({id:?}).ok_or_else(|| ::kortex::runtime::ToolError::with_detail(\"missing_input\", {id:?}))?)?,",
                id = i.id
            );
        } else {
            let _ = writeln!(
                out,
                "            {id}: match m.get({id:?}) {{ ::core::option::Option::Some(v) => ::core::option::Option::Some(<{t} as ::kortex::runtime::FromValue>::from_value(v)?), ::core::option::Option::None => ::core::option::Option::None }},",
                id = i.id
            );
        }
    }
    let _ = writeln!(out, "        }})\n    }}\n}}");
    let _ = writeln!(
        out,
        "impl Params {{\n    /// Missing keys keep their contract defaults.\n    ///\n    /// # Errors\n    /// `invalid_param` on a wrongly typed value.\n    pub fn from_map(m: &::kortex::runtime::ValueMap) -> ::core::result::Result<Self, ::kortex::runtime::ToolError> {{\n        #[allow(unused_mut)]\n        let mut s = Self::default();"
    );
    for p in &c.params {
        let _ = writeln!(
            out,
            "        if let ::core::option::Option::Some(v) = m.get({id:?}) {{ s.{id} = <{t} as ::kortex::runtime::FromValue>::from_value(v)?; }}",
            id = p.id,
            t = rust_type(&p.ty)
        );
    }
    let _ = writeln!(out, "        ::core::result::Result::Ok(s)\n    }}\n}}");
    let _ = writeln!(
        out,
        "impl Outputs {{\n    #[must_use]\n    pub fn into_map(self) -> ::kortex::runtime::ValueMap {{\n        #[allow(unused_mut)]\n        let mut m = ::kortex::runtime::ValueMap::new();"
    );
    for o in &c.outputs {
        let _ = writeln!(
            out,
            "        m.insert({id:?}.to_owned(), ::kortex::runtime::IntoValue::into_value(self.{id}));",
            id = o.id
        );
    }
    let _ = writeln!(out, "        m\n    }}\n}}");
    let _ = writeln!(
        out,
        "/// Dynamic entry used by hosts that only know the contract (UI, Flow, Mind, CLI).\n///\n/// # Errors\n/// Conversion errors, or whatever `engine::run` returns.\npub async fn run_map(inputs: &::kortex::runtime::ValueMap, params: &::kortex::runtime::ValueMap, cx: &mut ::kortex::runtime::Cx) -> ::core::result::Result<::kortex::runtime::ValueMap, ::kortex::runtime::ToolError> {{\n    let out = engine::run(Inputs::from_map(inputs)?, Params::from_map(params)?, cx).await?;\n    ::core::result::Result::Ok(out.into_map())\n}}"
    );

    // Levels available to the auto UI.
    let _ = write!(out, "pub const PARAM_LEVELS: &[(&str, &str)] = &[");
    for p in &c.params {
        let lvl = match p.level.unwrap_or(ParamLevel::Medium) {
            ParamLevel::Simple => "simple",
            ParamLevel::Medium => "medium",
            ParamLevel::Pro => "pro",
        };
        let _ = write!(out, "({:?}, {lvl:?}),", p.id);
    }
    let _ = writeln!(out, "];");
    Ok(out)
}

/// `kortex::contract!("tool.toml");`
#[proc_macro]
pub fn contract(input: TokenStream) -> TokenStream {
    let src = input.to_string();
    let raw = src.trim().trim_matches('"').to_owned();
    if raw.is_empty() || src.trim().len() == raw.len() {
        return compile_error("contract! attend un chemin littéral : contract!(\"tool.toml\")");
    }
    let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return compile_error("CARGO_MANIFEST_DIR indisponible");
    };
    let path = std::path::Path::new(&dir).join(&raw);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => return compile_error(&format!("{} illisible : {e}", path.display())),
    };
    let contract = match kortex_contract::parse(&text) {
        Ok(c) => c,
        Err(e) => return compile_error(&format!("contrat invalide : {e}")),
    };
    match generate(&contract, &raw) {
        Ok(code) => code
            .parse()
            .unwrap_or_else(|_| compile_error("code généré invalide")),
        Err(e) => compile_error(&e),
    }
}

/// `#[kortex::engine] pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError>`
#[proc_macro_attribute]
pub fn engine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let sig: String = item
        .to_string()
        .split('{')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("");
    let ok = sig.contains("pubasyncfnrun(")
        && sig.contains(":Inputs,")
        && sig.contains(":Params,")
        && (sig.contains("&mutCx)") || sig.contains("&mut::kortex::runtime::Cx)"))
        && sig.contains("->Result<Outputs,");
    if !ok {
        return compile_error(
            "#[engine] : la signature doit être `pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError>`",
        );
    }
    item
}
