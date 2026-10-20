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
export function cancelWorker() { if (active) active('cancelled'); cancelHosted(); }
// Engines of tools that are not bundled with the app run in a hidden iframe of the tool origin
// (`<run>/engines/host.html`): the app never executes tool code itself (ADR 0034).
let hostFrame = null, hostReady = null, hostOrigin = null, nextId = 1;
const pending = new Map();
function ensureHost(origin) {
    if (hostFrame && hostOrigin === origin) return hostReady;
    if (hostFrame) hostFrame.remove();
    hostOrigin = origin;
    hostReady = new Promise((resolve, reject) => {
        const frame = document.createElement('iframe');
        hostFrame = frame;
        frame.setAttribute('aria-hidden', 'true');
        frame.tabIndex = -1;
        frame.style.cssText = 'position:absolute;width:0;height:0;border:0;visibility:hidden';
        frame.referrerPolicy = 'origin';
        const timer = setTimeout(() => { hostFrame = null; reject('worker_failed'); }, 20000);
        window.addEventListener('message', e => {
            if (e.origin !== origin || e.source !== frame.contentWindow || !e.data) return;
            const d = e.data;
            if (d.kortex === 'engine-ready') { clearTimeout(timer); resolve(frame); }
            else if (d.kortex === 'engine-reply') {
                const p = pending.get(d.id);
                if (!p) return;
                pending.delete(d.id);
                clearTimeout(p.timer);
                if (d.error) p.reject(d.error); else p.resolve(d.value);
            }
        });
        frame.onload = () => frame.contentWindow.postMessage({kortex: 'engine-hello'}, origin);
        frame.src = origin + '/engines/host.html';
        document.body.appendChild(frame);
    });
    return hostReady;
}
function cancelHosted() {
    for (const [id, p] of pending) {
        clearTimeout(p.timer);
        pending.delete(id);
        if (hostFrame) hostFrame.contentWindow.postMessage({kortex: 'engine-cancel', id}, hostOrigin);
        p.reject('cancelled');
    }
}
export async function runHosted(origin, tool, inputs, params, timeout) {
    origin = globalThis.__KORTEX_RUN_ORIGIN || origin;
    let frame;
    try { frame = await ensureHost(origin); } catch (_) { throw 'worker_failed'; }
    cancelHosted();
    return new Promise((resolve, reject) => {
        const id = nextId++;
        const timer = setTimeout(() => {
            if (!pending.delete(id)) return;
            frame.contentWindow.postMessage({kortex: 'engine-cancel', id}, origin);
            reject('worker_timeout');
        }, timeout);
        pending.set(id, {resolve, reject, timer});
        try {
            frame.contentWindow.postMessage({kortex: 'engine-run', id, tool, inputs, params}, origin, [...buffers(inputs), ...buffers(params)]);
        } catch (_) {
            pending.delete(id);
            clearTimeout(timer);
            reject('worker_protocol');
        }
    });
}
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
    #[wasm_bindgen(catch, js_name = runHosted)]
    fn run_hosted_js(
        origin: &str,
        tool: &str,
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

/// Runs the engine of `tool` in the engine host of `origin` (the tool origin), like [`run`] but
/// for tools whose engine is not bundled with this app. Messages are typed, origin-checked and
/// buffers are transferred; cancellation and the delay end the run at once.
///
/// # Errors
/// Engine errors, cancellation, timeout, an unknown tool or host, or a malformed response.
pub async fn run_hosted<T: Serialize + serde::de::DeserializeOwned>(
    origin: &str,
    tool: &str,
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
    let promise = run_hosted_js(origin, tool, inputs, params, timeout_ms)
        .map_err(|_| failure("worker_failed"))?;
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
