//! Build a headless wasm engine, without Dioxus, for worker-enabled contracts.
use crate::fwd;
use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

const JS: &str = r#"
import init, { run } from './engine.js';
const ready = init();
function buffers(value, found = new Set()) {
    if (value instanceof Uint8Array) found.add(value.buffer);
    else if (value && typeof value === 'object') Object.values(value).forEach(v => buffers(v, found));
    return [...found];
}
self.onmessage = async ({data}) => {
    try {
        await ready;
        const reply = run(data.inputs, data.params);
        self.postMessage(reply, buffers(reply));
    } catch (_) {
        self.postMessage({Err: {code: 'worker_failed', detail: null}});
    }
};
"#;

pub fn build(root: &Path, tool_dir: &Path, id: &str) -> Result<(PathBuf, String, String), String> {
    let work = root.join(".kortex/worker").join(id);
    fs::create_dir_all(work.join("src")).map_err(|e| e.to_string())?;
    let crate_name = format!("tool-{}", id.replace('.', "-"));
    let manifest = format!(
        r#"[package]
name = "engine-{}"
version = "0.0.0"
edition = "2024"
[lib]
crate-type = ["cdylib"]
[dependencies]
tool = {{ package = {crate_name:?}, path = {:?} }}
kortex-runtime = {{ path = {:?} }}
serde = "1"
serde-wasm-bindgen = "0.6"
wasm-bindgen = "0.2.129"
[workspace]
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
"#,
        id.replace('.', "-"),
        fwd(tool_dir),
        fwd(&root.join("crates/kortex-runtime"))
    );
    fs::write(work.join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
    if !work.join("Cargo.lock").exists() {
        fs::copy(root.join("Cargo.lock"), work.join("Cargo.lock")).map_err(|e| e.to_string())?;
    }
    fs::write(
        work.join("src/lib.rs"),
        r#"
use kortex_runtime::{Cx, ValueMap, WorkerFailure, block_on};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(inputs: JsValue, params: JsValue) -> Result<JsValue, JsValue> {
    let response: Result<ValueMap, WorkerFailure> = (|| {
        let decode = |value| serde_wasm_bindgen::from_value(value).map_err(|_| WorkerFailure {
            code: "worker_protocol".to_owned(), detail: None,
        });
        let inputs = decode(inputs)?;
        let params = decode(params)?;
        block_on(tool::run_map(&inputs, &params, &mut Cx::new())).map_err(WorkerFailure::from)
    })();
    response.serialize(&serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true))
        .map_err(|_| JsValue::from_str("worker_protocol"))
}
"#,
    )
    .map_err(|e| e.to_string())?;
    let target = root.join(".kortex/target-worker");
    let status = Command::new("cargo")
        .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
        .current_dir(&work)
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("compilation Worker échouée : {id}"));
    }
    let wasm = target
        .join("wasm32-unknown-unknown/release")
        .join(format!("engine_{}.wasm", id.replace(['.', '-'], "_")));
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    fs::read(&wasm).map_err(|e| e.to_string())?.hash(&mut hash);
    JS.hash(&mut hash);
    let folder = format!("engine-{:016x}", hash.finish());
    let out = work.join(&folder);
    let status = Command::new("wasm-bindgen")
        .arg(&wasm)
        .args(["--target", "web", "--out-name", "engine", "--out-dir"])
        .arg(&out)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("wasm-bindgen Worker échoué : {id}"));
    }
    fs::write(out.join("worker.js"), JS).map_err(|e| e.to_string())?;
    let locale: toml::Table = fs::read_to_string(tool_dir.join("locales/en.toml"))
        .map_err(|e| e.to_string())?
        .parse()
        .map_err(|e: toml::de::Error| e.to_string())?;
    let codes = locale
        .get("errors")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(|table| table.keys())
        .filter(|key| {
            ![
                "cancelled",
                "worker_timeout",
                "worker_protocol",
                "worker_failed",
            ]
            .contains(&key.as_str())
        })
        .map(|key| format!("{key:?} => {key:?},"))
        .collect::<String>();
    Ok((out, folder, codes))
}
