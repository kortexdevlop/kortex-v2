//! One isolated Web Worker per run. Cancellation terminates CPU-bound Rust immediately.
use crate::WorkerFailure;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
let active = null;
function buffers(value, found = new Set()) {
    if (value instanceof Uint8Array) found.add(value.buffer);
    else if (value && typeof value === 'object') Object.values(value).forEach(v => buffers(v, found));
    return [...found];
}
export function cancelWorker() { if (active) active('cancelled'); }
export function runWorker(url, inputs, params, timeout) {
    cancelWorker();
    return new Promise((resolve, reject) => {
        const worker = new Worker(url, {type: 'module'});
        let timer;
        const finish = (error, value) => {
            clearTimeout(timer);
            worker.terminate();
            if (active === abort) active = null;
            if (error) reject(error); else resolve(value);
        };
        const abort = code => finish(code);
        active = abort;
        worker.onerror = () => finish('worker_failed');
        worker.onmessageerror = () => finish('worker_protocol');
        worker.onmessage = e => finish(null, e.data);
        timer = setTimeout(() => finish('worker_timeout'), timeout);
        try {
            worker.postMessage({inputs, params}, [...buffers(inputs), ...buffers(params)]);
        } catch (_) {
            finish('worker_protocol');
        }
    });
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = runWorker)]
    fn run_js(
        url: &str,
        inputs: JsValue,
        params: JsValue,
        timeout: u32,
    ) -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(js_name = cancelWorker)]
    pub fn cancel();
}

fn failure(code: &str) -> WorkerFailure {
    WorkerFailure {
        code: code.to_owned(),
        detail: None,
    }
}

/// Execute a typed tool invocation with transferable binary buffers.
///
/// # Errors
/// Engine errors, cancellation, timeout, loading failure or a malformed response.
pub async fn run<T: Serialize + serde::de::DeserializeOwned>(
    url: &str,
    inputs: T,
    params: T,
    timeout_ms: u32,
) -> Result<T, WorkerFailure> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    let inputs = inputs
        .serialize(&serializer)
        .map_err(|_| failure("worker_protocol"))?;
    let params = params
        .serialize(&serializer)
        .map_err(|_| failure("worker_protocol"))?;
    let promise = run_js(url, inputs, params, timeout_ms).map_err(|_| failure("worker_failed"))?;
    let response = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| {
            failure(match error.as_string().as_deref() {
                Some("cancelled") => "cancelled",
                Some("worker_timeout") => "worker_timeout",
                Some("worker_protocol") => "worker_protocol",
                _ => "worker_failed",
            })
        })?;
    serde_wasm_bindgen::from_value(response).map_err(|_| failure("worker_protocol"))?
}
