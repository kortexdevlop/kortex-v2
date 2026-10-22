// Origin-local byte storage adapter. Domain project validation remains in Rust.
const LIMIT = 1024 * 1024;
const validId = id => typeof id === 'string' && /^[A-Za-z0-9_-]{1,96}$/.test(id);
const validRevision = value => typeof value === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(value) && BigInt(value) <= 18446744073709551615n;
function record(value) {
  if (!value || !validRevision(value.revision) || !(value.bytes instanceof Uint8Array) || value.bytes.byteLength > LIMIT) throw 'Corrupt';
  return value;
}
function open(namespace) {
  if (!validId(namespace)) return Promise.reject('InvalidId');
  return new Promise((resolve, reject) => {
    let request;
    let failed = false;
    try { request = globalThis.indexedDB.open(`kortex:projects:v1:${namespace}`, 1); }
    catch { reject('Unavailable'); return; }
    request.onupgradeneeded = () => request.result.createObjectStore('snapshots');
    request.onerror = () => reject('Unavailable');
    request.onblocked = () => { failed = true; reject('Unavailable'); };
    request.onsuccess = () => {
      if (failed) request.result.close();
      else resolve(request.result);
    };
  });
}
async function transaction(namespace, id, expected, next) {
  if (!validId(id)) throw 'InvalidId';
  const db = await open(namespace);
  return new Promise((resolve, reject) => {
    let tx;
    let result = null;
    let failure = 'Unavailable';
    try {
      tx = db.transaction('snapshots', next ? 'readwrite' : 'readonly');
      tx.oncomplete = () => { db.close(); resolve(result); };
      tx.onabort = () => { db.close(); reject(failure); };
      const store = tx.objectStore('snapshots');
      const get = store.get(id);
      get.onsuccess = () => {
        try {
          const current = get.result === undefined ? null : record(get.result);
          if (!next) { result = current; return; }
          if ((current?.revision ?? null) !== expected) throw 'Conflict';
          store.put(next, id);
        } catch (error) {
          failure = ['Conflict', 'Corrupt'].includes(error) ? error : 'Unavailable';
          tx.abort();
        }
      };
    } catch {
      db.close();
      reject('Unavailable');
    }
  });
}
export function loadSnapshot(namespace, id) {
  return transaction(namespace, id, null, null);
}
export function casSnapshot(namespace, id, expected, revision, bytes) {
  if (!validRevision(revision) || (expected !== null && !validRevision(expected))) return Promise.reject('Corrupt');
  if (!(bytes instanceof Uint8Array)) return Promise.reject('Corrupt');
  if (bytes.byteLength > LIMIT) return Promise.reject('TooLarge');
  if (expected === null ? revision !== '0' : BigInt(revision) !== BigInt(expected) + 1n) return Promise.reject('Conflict');
  return transaction(namespace, id, expected, { revision, bytes: Uint8Array.from(bytes) });
}
