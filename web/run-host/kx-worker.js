// Generic engine worker of the tool origin (ADR 0035). The worker script of a tool,
// `/<tool>/engine/worker.js`, imports this module and calls `serve('<tool>')`: one worker per run,
// which loads `/<tool>/engine/engine.wasm`, provides the `kx_host` services and converts between
// the value objects of the interface (file bytes are Uint8Array) and the engine ABI v1 frames.
// The tool identifier is an argument, not part of this module's URL: the offline cache serves
// the module without its query string.
//
// Frame: u32 LE JSON length, the JSON, then the binary blobs back to back. File bytes are blobs:
// `{File:{name,mime,blob:i}}`; the JSON lists the blob lengths in `blobs`.
const TOOL_ID = /^[a-z0-9]+\.[a-z0-9-]+$/;
const ENGINE_ABI = 1;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function failure(code, detail = null) {
  return {Err: {code, detail}};
}

function encodeFile(file, blobs) {
  blobs.push(file.bytes);
  return {name: file.name, mime: file.mime, blob: blobs.length - 1};
}

function encodeValue(value, blobs) {
  const [tag, inner] = Object.entries(value)[0];
  if (tag === 'File') return {File: encodeFile(inner, blobs)};
  if (tag === 'Files') return {Files: inner.map((f) => encodeFile(f, blobs))};
  if (tag === 'Number' && !Number.isFinite(inner)) {
    return {Number: Number.isNaN(inner) ? 'NaN' : inner > 0 ? 'Inf' : '-Inf'};
  }
  if (tag === 'Int') return {Int: Number(inner)};
  return {[tag]: inner};
}

function encodeMap(map, blobs) {
  return Object.fromEntries(Object.entries(map || {}).map(([k, v]) => [k, encodeValue(v, blobs)]));
}

function encodeRequest(inputs, params) {
  const blobs = [];
  const json = encoder.encode(JSON.stringify({
    inputs: encodeMap(inputs, blobs),
    params: encodeMap(params, blobs),
    blobs: blobs.map((b) => b.length),
  }));
  const out = new Uint8Array(4 + json.length + blobs.reduce((n, b) => n + b.length, 0));
  new DataView(out.buffer).setUint32(0, json.length, true);
  out.set(json, 4);
  let at = 4 + json.length;
  for (const blob of blobs) {
    out.set(blob, at);
    at += blob.length;
  }
  return out;
}

function decodeResponse(bytes) {
  const jsonLength = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(0, true);
  const head = JSON.parse(decoder.decode(bytes.subarray(4, 4 + jsonLength)));
  let at = 4 + jsonLength;
  const blobs = head.blobs.map((length) => {
    const blob = bytes.slice(at, at + length);
    at += length;
    return blob;
  });
  if (at !== bytes.length) throw new Error('bytes after the last blob');
  if (head.Err) return failure(String(head.Err.code), head.Err.detail ?? null);
  const file = (f) => ({name: f.name, mime: f.mime, bytes: blobs[f.blob]});
  const value = (v) => {
    const [tag, inner] = Object.entries(v)[0];
    if (tag === 'File') return {File: file(inner)};
    if (tag === 'Files') return {Files: inner.map(file)};
    if (tag === 'Number' && typeof inner === 'string') {
      return {Number: inner === 'NaN' ? NaN : inner === 'Inf' ? Infinity : -Infinity};
    }
    return {[tag]: inner};
  };
  return {Ok: Object.fromEntries(Object.entries(head.Ok).map(([k, v]) => [k, value(v)]))};
}

function transferables(reply) {
  const found = new Set();
  const walk = (v) => {
    if (v instanceof Uint8Array) found.add(v.buffer);
    else if (v && typeof v === 'object') Object.values(v).forEach(walk);
  };
  walk(reply);
  return [...found];
}

async function instantiate(tool) {
  if (!TOOL_ID.test(tool)) throw new Error('invalid tool');
  const response = await fetch(`/${tool}/engine/engine.wasm`, {credentials: 'omit'});
  if (!response.ok) throw new Error('engine unavailable');
  const bytes = await response.arrayBuffer();
  const module = await WebAssembly.compile(bytes);
  let memory = null;
  const host = {
    now_ms: () => BigInt(Date.now()),
    fill_random: (ptr, len) => {
      for (let done = 0; done < len; done += 65536) {
        crypto.getRandomValues(new Uint8Array(memory.buffer, ptr + done, Math.min(65536, len - done)));
      }
      return 0;
    },
    is_cancelled: () => 0,
    progress: () => {},
  };
  // Imports outside `kx_host` (residue of third-party crates) fail only if the engine calls them.
  const imports = {kx_host: host};
  for (const {module: m, name, kind} of WebAssembly.Module.imports(module)) {
    if (m === 'kx_host') continue;
    if (kind !== 'function') throw new Error('unsupported import');
    (imports[m] ||= {})[name] = () => { throw new Error(`unavailable import ${m}.${name}`); };
  }
  const instance = await WebAssembly.instantiate(module, imports);
  memory = instance.exports.memory;
  return instance.exports;
}

function run(engine, request) {
  if (engine.kx_abi() !== ENGINE_ABI) throw new Error('unsupported engine ABI');
  const length = request.length;
  const pointer = engine.kx_alloc(length);
  new Uint8Array(engine.memory.buffer, pointer, length).set(request);
  const packed = engine.kx_run(pointer, length);
  const outPointer = Number(packed >> 32n);
  const outLength = Number(packed & 0xffffffffn);
  const response = new Uint8Array(engine.memory.buffer, outPointer, outLength).slice();
  engine.kx_free(outPointer, outLength);
  engine.kx_free(pointer, length);
  return response;
}

export function serve(tool) {
  self.onmessage = async ({data}) => {
    let reply;
    try {
      const engine = await instantiate(tool);
      reply = decodeResponse(run(engine, encodeRequest(data.inputs, data.params)));
    } catch (error) {
      reply = failure('worker_failed', String((error && error.message) || error));
    }
    self.postMessage(reply, transferables(reply));
  };
}
