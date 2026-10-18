// Runs headless tool engines (dist/engines/<id>/worker.js) in a real browser Worker.
// Needs `cargo xtask build-engine <ids…>` first. CHROME_PATH may point at an installed Chrome.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { chromium } from 'playwright';

const dist = resolve(import.meta.dirname, '../../dist/engines');
assert.ok(existsSync(dist), 'dist/engines missing: run `cargo xtask build-engine <ids…>` first');
const types = { '.js': 'text/javascript', '.wasm': 'application/wasm', '.html': 'text/html' };
const server = createServer((request, response) => {
  const path = resolve(dist, '.' + decodeURIComponent(new URL(request.url, 'http://x').pathname));
  if (path !== dist && !path.startsWith(dist + sep)) return response.writeHead(403).end();
  if (!existsSync(path) || !statSync(path).isFile()) {
    if (request.url === '/') return response.writeHead(200, { 'content-type': 'text/html' }).end('<!doctype html>');
    return response.writeHead(404).end();
  }
  response.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' }).end(readFileSync(path));
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || undefined });
// One request per case: [tool id, inputs, params, check on the Ok value or the Err].
const cases = [
  ['text.cleaner', { text: { Text: '  hello   world  ' } }, {}, r => assert.equal(r.Ok.text.Text, 'hello world')],
  ['text.cleaner', { text: { Text: '' } }, {}, r => assert.equal(r.Err.code, 'empty_input')],
  ['text.word-counter', { text: { Text: 'one two three' } }, {}, r => assert.ok(r.Ok, JSON.stringify(r))],
  ['secure.hash-calculator', { text: { Text: 'abc' } }, { algorithm: { Text: 'sha256' } }, r => assert.ok(JSON.stringify(r).includes('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'), JSON.stringify(r))],
];
const results = [];
try {
  const page = await browser.newPage();
  await page.goto(origin + '/');
  for (const [id, inputs, params, check] of cases) {
    const reply = await page.evaluate(({ origin, id, inputs, params }) => new Promise((resolve, reject) => {
      const worker = new Worker(`${origin}/${id}/worker.js`, { type: 'module' });
      const timer = setTimeout(() => reject(new Error('worker timeout')), 20000);
      worker.onmessage = event => { clearTimeout(timer); worker.terminate(); resolve(event.data); };
      worker.onerror = event => { clearTimeout(timer); reject(new Error(event.message)); };
      worker.postMessage({ inputs, params });
    }), { origin, id, inputs, params });
    check(reply);
    results.push(id);
  }
  console.log(`engines OK: ${results.join(', ')}`);
} finally {
  await browser.close();
  server.close();
}
