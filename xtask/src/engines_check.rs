//! `cargo xtask engines-check [id…|all]`: every engine must answer the same through its
//! `engine.wasm` (hosted by `kortex-engine-host`) as natively (ADR 0035).
//!
//! Requests come from the contract (defaults, every option of a list, both booleans, range ends),
//! with generated PNG/PDF files for file ports and optional extra requests in
//! `tools/<u>/<n>/tests/fixtures/requests.json` (`[{"inputs": {...}, "params": {...}}]`, values in
//! the runtime's serde form). Both answers are re-encoded as canonical frames and compared byte
//! for byte; an engine whose native answer differs between two runs (entropy, clock) is compared
//! by shape instead.
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use kortex_contract::{Input, Param, PortType, ToolContract};
use kortex_engine_host::EngineModule;
use kortex_runtime::{CancelToken, FileData, Table, ToolError, Value, ValueMap, frame};

const TEXT: &str = "Bonjour le monde ! Ça va ? 123 — Hello, World.\nDeuxième ligne : Test 4.5";

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
    out
}

/// A 24×16 RGB gradient PNG.
fn png() -> Result<Vec<u8>, String> {
    use flate2::{Compression, write::ZlibEncoder};
    let (w, h) = (24_u32, 16_u32);
    let mut raw = Vec::new();
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            raw.extend_from_slice(&[(x * 10) as u8, (y * 15) as u8, ((x + y) * 6) as u8]);
        }
    }
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(&raw).map_err(|e| e.to_string())?;
    let idat = z.finish().map_err(|e| e.to_string())?;
    let mut header = w.to_be_bytes().to_vec();
    header.extend_from_slice(&h.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    out.extend(chunk(b"IHDR", &header));
    out.extend(chunk(b"IDAT", &idat));
    out.extend(chunk(b"IEND", &[]));
    Ok(out)
}

/// A one-page PDF with a line of text.
fn pdf(text: &str) -> Vec<u8> {
    let stream = format!("BT /F1 24 Tf 72 700 Td ({text}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn file(mime: &str, n: usize) -> Result<FileData, String> {
    let (name, mime, bytes) = if mime.starts_with("image/") {
        (format!("sample-{n}.png"), "image/png".to_owned(), png()?)
    } else if mime == "application/pdf" {
        (
            format!("sample-{n}.pdf"),
            mime.to_owned(),
            pdf(&format!("Page {n}")),
        )
    } else {
        (
            format!("sample-{n}.txt"),
            "text/plain".to_owned(),
            TEXT.as_bytes().to_vec(),
        )
    };
    Ok(FileData { name, mime, bytes })
}

fn input_value(input: &Input) -> Result<Value, String> {
    Ok(match &input.ty {
        PortType::Number | PortType::Duration => Value::Number(1.5),
        PortType::Integer => Value::Int(3),
        PortType::Bool => Value::Bool(false),
        PortType::Table => Value::Table(Table {
            columns: vec!["a".to_owned(), "b".to_owned()],
            rows: vec![vec!["1".to_owned(), "2".to_owned()]],
        }),
        PortType::File(mime) => Value::File(file(mime, 1)?),
        PortType::Files(mime) => Value::Files(vec![file(mime, 1)?, file(mime, 2)?]),
        PortType::Color => Value::Text("#336699".to_owned()),
        PortType::Date => Value::Text("2026-10-21".to_owned()),
        PortType::DateTime => Value::Text("2026-10-21T12:00:00Z".to_owned()),
        PortType::Url => Value::Text("https://kortex.site/page?a=1".to_owned()),
        _ => Value::Text(TEXT.to_owned()),
    })
}

fn from_toml(ty: &PortType, value: &toml::Value) -> Option<Value> {
    match (ty, value) {
        (PortType::Bool, toml::Value::Boolean(b)) => Some(Value::Bool(*b)),
        (PortType::Integer, toml::Value::Integer(i)) => Some(Value::Int(*i)),
        (PortType::Number, toml::Value::Integer(i)) => Some(Value::Number(*i as f64)),
        (PortType::Number, toml::Value::Float(f)) => Some(Value::Number(*f)),
        (PortType::Integer, toml::Value::Float(f)) => Some(Value::Int(*f as i64)),
        (_, toml::Value::String(s)) => Some(Value::Text(s.clone())),
        _ => None,
    }
}

fn param_values(param: &Param) -> Vec<Value> {
    let default = param
        .default
        .as_ref()
        .and_then(|d| from_toml(&param.ty, d))
        .or(match param.ty {
            PortType::Bool => Some(Value::Bool(false)),
            PortType::Integer => Some(Value::Int(1)),
            PortType::Number => Some(Value::Number(1.0)),
            _ => None,
        });
    let mut values: Vec<Value> = default.into_iter().collect();
    match &param.ty {
        PortType::Bool => values.extend([Value::Bool(true), Value::Bool(false)]),
        PortType::Enum => values.extend(
            param
                .options
                .iter()
                .flatten()
                .map(|o| Value::Text(o.clone())),
        ),
        PortType::Integer | PortType::Number => {
            for end in param.range.iter().flatten() {
                values.push(if param.ty == PortType::Integer {
                    Value::Int(*end as i64)
                } else {
                    Value::Number(*end)
                });
            }
        }
        _ => {}
    }
    let mut seen = BTreeSet::new();
    values.retain(|v| seen.insert(format!("{v:?}")));
    values
}

type Request = (ValueMap, ValueMap);

fn requests(contract: &ToolContract, dir: &Path) -> Result<Vec<Request>, String> {
    let mut inputs = ValueMap::new();
    for input in &contract.inputs {
        inputs.insert(input.id.clone(), input_value(input)?);
    }
    let mut base = ValueMap::new();
    for param in &contract.params {
        if let Some(value) = param_values(param).into_iter().next() {
            base.insert(param.id.clone(), value);
        }
    }
    let mut all = vec![(inputs.clone(), base.clone())];
    for param in &contract.params {
        for value in param_values(param).into_iter().skip(1) {
            let mut params = base.clone();
            params.insert(param.id.clone(), value);
            all.push((inputs.clone(), params));
        }
    }
    let empty: ValueMap = contract
        .inputs
        .iter()
        .filter(|i| matches!(i.ty, PortType::Text | PortType::RichText | PortType::Json))
        .map(|i| (i.id.clone(), Value::Text(String::new())))
        .collect();
    if !empty.is_empty() {
        all.push((empty, base.clone()));
    }
    let fixtures = dir.join("tests/fixtures/requests.json");
    if fixtures.is_file() {
        let list: Vec<serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(&fixtures).map_err(|e| e.to_string())?)
                .map_err(|e| format!("{} : {e}", fixtures.display()))?;
        for entry in list {
            let part = |key: &str| -> Result<ValueMap, String> {
                entry.get(key).map_or_else(
                    || Ok(ValueMap::new()),
                    |v| {
                        serde_json::from_value(v.clone())
                            .map_err(|e| format!("{} : {key} : {e}", fixtures.display()))
                    },
                )
            };
            all.push((part("inputs")?, part("params")?));
        }
    }
    all.truncate(40);
    Ok(all)
}

fn native(binary: &Path, request: &[u8]) -> Result<Vec<u8>, String> {
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("{} : {e}", binary.display()))?;
    let mut stdin = child.stdin.take().ok_or("stdin indisponible")?;
    let payload = request.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&payload));
    let mut out = Vec::new();
    child
        .stdout
        .take()
        .ok_or("stdout indisponible")?
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    let _ = writer.join();
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(out)
    } else {
        Err(format!("moteur natif terminé en erreur ({status})"))
    }
}

fn outcome(frame_bytes: &[u8]) -> Result<Result<ValueMap, ToolError>, String> {
    Ok(frame::decode_response(frame_bytes).map_err(frame::Failure::into_tool_error))
}

/// What a result looks like without its volatile content: kind and names only.
fn shape(result: &Result<ValueMap, ToolError>) -> String {
    match result {
        Ok(map) => map
            .iter()
            .map(|(k, v)| {
                let kind = match v {
                    Value::Text(_) => "text",
                    Value::Number(_) => "number",
                    Value::Int(_) => "int",
                    Value::Bool(_) => "bool",
                    Value::File(_) => "file",
                    Value::Files(_) => "files",
                    Value::Table(_) => "table",
                };
                format!("{k}:{kind}")
            })
            .collect::<Vec<_>>()
            .join(","),
        Err(e) => format!("err:{}", e.code),
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    // Debug builds of the interpreter use a lot of stack on large engines.
    let args = args.to_vec();
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || check(&args))
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "engines-check interrompu (panique)".to_owned())?
}

fn check(args: &[String]) -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let all = args.is_empty() || args.iter().any(|a| a == "all");
    let mut failures = Vec::new();
    let (mut checked, mut volatile) = (0_usize, 0_usize);
    println!("{:<28} {:>4} {:>4}  résultat", "outil", "req", "ok");
    for tool in crate::tools::discover()? {
        let contract = tool
            .contract
            .map_err(|e| format!("{} : {e}", tool.dir.display()))?;
        let id = contract.tool.id.clone();
        if !all && !args.contains(&id) {
            continue;
        }
        let wasm_path = root.join("dist/engines").join(&id).join("engine.wasm");
        let wasm = fs::read(&wasm_path).map_err(|e| {
            format!(
                "{id} : {} : {e} (lance `cargo xtask build-engine {id}`)",
                wasm_path.display()
            )
        })?;
        let module = EngineModule::load(&wasm).map_err(|e| format!("{id} : {e}"))?;
        let binary = crate::engine::build_native(&root, &root.join(&tool.dir), &id)?;
        let timeout = Duration::from_secs(u64::from(contract.limits.timeout_s.unwrap_or(120)));
        let (mut ok, mut count, mut non_deterministic) = (0_usize, 0_usize, false);
        let mut mismatch = None;
        for (inputs, params) in requests(&contract, &root.join(&tool.dir))? {
            count += 1;
            let request = frame::encode_request(&inputs, &params);
            let first = outcome(&native(&binary, &request)?)?;
            let second = outcome(&native(&binary, &request)?)?;
            let wasm_result = module.run(&inputs, &params, &CancelToken::default(), Some(timeout));
            if first.is_ok() {
                ok += 1;
            }
            let same = if first == second {
                frame::encode_response(&first) == frame::encode_response(&wasm_result)
            } else {
                non_deterministic = true;
                shape(&first) == shape(&wasm_result)
            };
            if !same && mismatch.is_none() {
                mismatch = Some(format!(
                    "natif {first:?}\n    wasm  {wasm_result:?}",
                    first = shape_or_value(&first),
                    wasm_result = shape_or_value(&wasm_result)
                ));
            }
        }
        checked += 1;
        volatile += usize::from(non_deterministic);
        let verdict = match &mismatch {
            None if non_deterministic => "identique (forme seulement : entropie/horloge)",
            None => "identique",
            Some(_) => "DIFFÉRENT",
        };
        println!("{id:<28} {count:>4} {ok:>4}  {verdict}");
        if let Some(detail) = mismatch {
            failures.push(format!("{id}\n    {detail}"));
        }
    }
    if checked == 0 {
        return Err("aucun outil correspondant".to_owned());
    }
    println!("{checked} outil(s) contrôlé(s), {volatile} comparé(s) par la forme");
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "natif et wasm diffèrent :\n  {}",
            failures.join("\n  ")
        ))
    }
}

fn shape_or_value(result: &Result<ValueMap, ToolError>) -> String {
    let text = format!("{result:?}");
    if text.len() > 300 {
        format!("{}… [{}]", &text[..300], shape(result))
    } else {
        text
    }
}
