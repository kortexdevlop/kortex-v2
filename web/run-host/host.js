// Engine host of the tool origin (ADR 0034). An app (Flow, Mind) loads this page in a hidden
// iframe and asks it to run the headless engine of a tool: `/<tool>/engine/worker.js`, a Worker of
// THIS origin, so tool code never runs with the privileges of the app. Messages are typed, the
// sender must be the parent window of an allowed origin, and every reply names its target origin.
const ALLOWED = __ALLOWED_ORIGINS__;
const TOOL_ID = /^[a-z0-9]+\.[a-z0-9-]+$/;
const runs = new Map();

function buffers(value, found = new Set()) {
  if (value instanceof Uint8Array) found.add(value.buffer);
  else if (value && typeof value === 'object') Object.values(value).forEach((v) => buffers(v, found));
  return [...found];
}

function finish(id, error, value) {
  const run = runs.get(id);
  if (!run) return;
  runs.delete(id);
  run.worker.terminate();
  const message = {kortex: 'engine-reply', id};
  if (error) message.error = error; else message.value = value;
  parent.postMessage(message, run.origin, error ? [] : buffers(value));
}

addEventListener('message', (e) => {
  if (e.source !== parent || !ALLOWED.includes(e.origin)) return;
  const d = e.data;
  if (!d || typeof d !== 'object') return;
  if (d.kortex === 'engine-hello') {
    parent.postMessage({kortex: 'engine-ready'}, e.origin);
  } else if (d.kortex === 'engine-cancel') {
    finish(d.id, 'cancelled');
  } else if (d.kortex === 'engine-run') {
    if (typeof d.tool !== 'string' || !TOOL_ID.test(d.tool) || !Number.isSafeInteger(d.id) || runs.has(d.id)) {
      parent.postMessage({kortex: 'engine-reply', id: d.id, error: 'worker_protocol'}, e.origin);
      return;
    }
    const worker = new Worker(`/${d.tool}/engine/worker.js`, {type: 'module'});
    runs.set(d.id, {worker, origin: e.origin});
    worker.onmessage = (m) => finish(d.id, null, m.data);
    worker.onerror = () => finish(d.id, 'worker_failed');
    worker.onmessageerror = () => finish(d.id, 'worker_protocol');
    try {
      worker.postMessage({inputs: d.inputs, params: d.params}, [...buffers(d.inputs), ...buffers(d.params)]);
    } catch (_) {
      finish(d.id, 'worker_protocol');
    }
  }
});
