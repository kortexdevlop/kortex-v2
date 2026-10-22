// Real browser acceptance of the IndexedDB byte adapter, independent of product UI.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { chromium } from 'playwright';

const source = readFileSync(new URL('../../crates/kortex-platform/src/project-store.js', import.meta.url));
const server = createServer((request, response) => {
  if (request.url === '/project-store.js') response.writeHead(200, {'content-type': 'text/javascript'}).end(source);
  else response.writeHead(200, {'content-type': 'text/html'}).end('<!doctype html><title>Project storage fixture</title>');
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser;
try {
  browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || undefined });
  const context = await browser.newContext();
  const origin = `http://127.0.0.1:${server.address().port}`;
  const first = await context.newPage();
  const second = await context.newPage();
  for (const page of [first, second]) {
    await page.goto(origin);
    await page.evaluate(async () => { globalThis.adapter = await import('/project-store.js'); });
  }
  assert.equal(await first.evaluate(() => adapter.loadSnapshot('test', 'project')), null);
  await first.evaluate(() => adapter.casSnapshot('test', 'project', null, '0', new Uint8Array([1, 2, 3])));
  const writes = await Promise.all([first, second].map((page, index) => page.evaluate(async index => {
    try {
      await adapter.casSnapshot('test', 'project', '0', '1', new Uint8Array([index + 10]));
      return 'saved';
    } catch (e) { return e; }
  }, index)));
  assert.deepEqual(writes.sort(), ['Conflict', 'saved']);
  await first.reload();
  const snapshot = await first.evaluate(async () => {
    const {loadSnapshot} = await import('/project-store.js');
    const value = await loadSnapshot('test', 'project');
    return { revision: value.revision, bytes: [...value.bytes] };
  });
  assert.equal(snapshot.revision, '1');
  assert.ok([10, 11].includes(snapshot.bytes[0]));
  assert.equal(await second.evaluate(() => adapter.loadSnapshot('other', 'project')), null);
  const rejected = await second.evaluate(async () => {
    const attempt = async fn => { try { await fn(); return 'unexpected success'; } catch (e) { return e; } };
    return Promise.all([
      attempt(() => adapter.casSnapshot('test', 'project', null, '0', new Uint8Array())),
      attempt(() => adapter.casSnapshot('test', 'project', '1', '3', new Uint8Array())),
      attempt(() => adapter.casSnapshot('test', '../bad', null, '0', new Uint8Array())),
      attempt(() => adapter.casSnapshot('test', 'large', null, '0', new Uint8Array(1024 * 1024 + 1))),
      attempt(() => adapter.casSnapshot('test', 'overflow', '18446744073709551615', '18446744073709551616', new Uint8Array())),
    ]);
  });
  assert.deepEqual(rejected, ['Conflict', 'Conflict', 'InvalidId', 'TooLarge', 'Corrupt']);
  // Inject a corrupt local record and a high revision to test exact u64 handling.
  await second.evaluate(() => new Promise((resolve, reject) => {
    const request = indexedDB.open('kortex:projects:v1:test', 1);
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction('snapshots', 'readwrite');
      tx.objectStore('snapshots').put({revision: 'bad', bytes: new Uint8Array()}, 'corrupt');
      tx.objectStore('snapshots').put({revision: '9007199254740992', bytes: new Uint8Array()}, 'high');
      tx.oncomplete = () => { db.close(); resolve(); };
      tx.onabort = () => { db.close(); reject(tx.error); };
    };
  }));
  assert.equal(await second.evaluate(async () => {
    try { await adapter.loadSnapshot('test', 'corrupt'); } catch (e) { return e; }
  }), 'Corrupt');
  await second.evaluate(() => adapter.casSnapshot('test', 'high', '9007199254740992', '9007199254740993', new Uint8Array([42])));
  assert.equal(await second.evaluate(async () => (await adapter.loadSnapshot('test', 'high')).revision), '9007199254740993');
  console.log('project-store: cross-tab CAS, reload, isolation, corruption, bounds and exact u64 revisions passed');
} finally {
  await browser?.close();
  await new Promise(resolve => server.close(resolve));
}
