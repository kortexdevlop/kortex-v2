// Real Chromium smoke test against built engines and the production platform bridge.
// Usage: NODE_PATH=<directory containing playwright> node scripts/tests/worker-smoke.mjs
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { access, readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { resolve, extname } from 'node:path';
const { chromium } = createRequire(import.meta.url)('playwright');
const root = resolve(import.meta.dirname, '../..');
const bridge = (await readFile(resolve(root, 'crates/kortex-platform/src/worker.rs'), 'utf8')).split('r#"')[1].split('"#')[0];
const engines = {};
for (const id of ['secure.hash-calculator', 'pdf.merge']) {
  await access(resolve(root, 'dist/run', id, 'engine/engine.wasm')).catch(() => assert.fail(`Build ${id} first with cargo xtask build`));
  engines[id] = `/${id}/engine/worker.js`;
}
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/') { res.setHeader('Content-Type', 'text/html'); res.end('<title>Worker smoke</title>'); return; }
    if (req.url === '/bridge.js') { res.setHeader('Content-Type', 'text/javascript'); res.end(bridge); return; }
    let path = resolve(root, 'dist/run', '.' + new URL(req.url, 'http://local').pathname);
    if (req.url.endsWith('/')) path = resolve(path, 'index.html');
    if (!path.startsWith(resolve(root, 'dist/run') + '/')) throw Error('path');
    res.setHeader('Content-Type', extname(path) === '.wasm' ? 'application/wasm' : extname(path) === '.html' ? 'text/html' : 'text/javascript');
    res.end(await readFile(path));
  } catch { res.statusCode = 404; res.end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
let browser;
try {
  browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.evaluate(async () => { window.bridge = await import('/bridge.js'); });
  const report = await page.evaluate(async engines => {
    const call = (id, inputs, params = {}, timeout = 120000) => bridge.runWorker(engines[id], inputs, params, timeout);
    const hash = async inputs => (await call('secure.hash-calculator', inputs)).Ok.hash.Text;
    const results = {};
    results.abc = await hash({ text: { Text: 'abc' } });
    results.emptyError = (await call('secure.hash-calculator', {})).Err.code;
    results.afterError = await hash({ text: { Text: 'abc' } });
    // 64 MiB stresses transfer and wasm copies without claiming 2 GiB support.
    const bytes = new Uint8Array(64 * 1024 * 1024).fill(97);
    let ticks = 0;
    const timer = setInterval(() => ticks++, 10);
    const start = performance.now();
    results.largeHash = await hash({ file: { File: { name: 'large.bin', mime: 'application/octet-stream', bytes } } });
    clearInterval(timer);
    results.largeMs = Math.round(performance.now() - start);
    results.uiTicks = ticks;
    results.transferred = bytes.byteLength === 0;
    // A valid small PDF with its own xref (no fixture dependencies).
    function pdf(label) {
      let text = '%PDF-1.4\n';
      const offsets = [0];
      const stream = `BT /F1 12 Tf 10 10 Td (${label}) Tj ET`;
      const objects = ['<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>', '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>', '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>', `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`];
      objects.forEach((obj, i) => { offsets.push(text.length); text += `${i + 1} 0 obj\n${obj}\nendobj\n`; });
      const xref = text.length;
      text += `xref\n0 6\n0000000000 65535 f \n` + offsets.slice(1).map(n => `${String(n).padStart(10, '0')} 00000 n \n`).join('');
      text += `trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
      return { name: label + '.pdf', mime: 'application/pdf', bytes: new TextEncoder().encode(text) };
    }
    const merged = await call('pdf.merge', { files: { Files: [pdf('A'), pdf('B')] } });
    results.pdfBytes = Array.from(merged.Ok.merged.File.bytes);
    // Not a PDF at all (wrong signature), then a file with the right signature that cannot be parsed.
    results.pdfError = (await call('pdf.merge', { files: { Files: [pdf('A'), { name: 'bad.pdf', mime: 'application/pdf', bytes: new Uint8Array([1, 2]) }] } })).Err.code;
    results.pdfBroken = (await call('pdf.merge', { files: { Files: [pdf('A'), { name: 'broken.pdf', mime: 'application/pdf', bytes: new TextEncoder().encode('%PDF-1.4 broken') }] } })).Err.code;
    const long = call('secure.hash-calculator', { file: { File: { name: 'cancel.bin', mime: 'application/octet-stream', bytes: new Uint8Array(64 * 1024 * 1024) } } }).catch(e => e);
    setTimeout(() => bridge.cancelWorker(), 20);
    results.cancel = await long;
    results.afterCancel = await hash({ text: { Text: 'abc' } });
    results.timeout = await call('secure.hash-calculator', { text: { Text: 'abc' } }, {}, 0).catch(e => e);
    results.afterTimeout = await hash({ text: { Text: 'abc' } });
    results.missingWorker = await bridge.runWorker('/missing.js', {}, {}, 1000).catch(e => e);
    return results;
  }, engines);
  const abc = createHash('sha256').update('abc').digest('hex');
  for (const key of ['abc', 'afterError', 'afterCancel', 'afterTimeout']) assert.equal(report[key], abc, key);
  assert.equal(report.largeHash, createHash('sha256').update(Buffer.alloc(64 * 1024 * 1024, 97)).digest('hex'));
  assert.ok(report.uiTicks > 0, 'UI event loop remains active');
  assert.equal(report.transferred, true, 'input ArrayBuffer ownership transferred');
  assert.equal(report.emptyError, 'missing_input');
  assert.equal(report.pdfError, 'file_type_mismatch');
  assert.equal(report.pdfBroken, 'invalid_pdf');
  assert.equal(report.cancel, 'cancelled');
  assert.equal(report.timeout, 'worker_timeout');
  assert.equal(report.missingWorker, 'worker_failed');
  const pdf = Buffer.from(report.pdfBytes).toString('latin1');
  assert.ok(pdf.startsWith('%PDF-'));
  assert.match(pdf, /\/Count 2\b/);
  // Prove the production service worker serves a cold engine with all network disabled.
  await page.evaluate(async () => {
    const registration = await navigator.serviceWorker.register('/secure.hash-calculator/sw.js');
    const worker = registration.installing || registration.waiting || registration.active;
    if (worker.state !== 'activated') await new Promise((resolve, reject) => worker.addEventListener('statechange', () => {
      if (worker.state === 'activated') resolve();
      if (worker.state === 'redundant') reject(Error('Offline cache installation failed'));
    }));
  });
  await page.goto(`http://127.0.0.1:${server.address().port}/secure.hash-calculator/?lang=en&mode=pro`);
  await page.waitForFunction(() => navigator.serviceWorker.controller);
  // The bridge is loaded online only as a test helper; it is not a product asset.
  await page.evaluate(async () => { window.testBridge = await import('/bridge.js'); });
  await page.context().setOffline(true);
  report.offlineHash = await page.evaluate(async url => (await testBridge.runWorker(url, { text: { Text: 'abc' } }, {}, 10000)).Ok.hash.Text, engines['secure.hash-calculator']);
  assert.equal(report.offlineHash, abc);
  await page.reload();
  await page.getByRole('heading', { name: 'Hash Calculator', exact: true }).waitFor();
  report.offlineReload = true;
  delete report.pdfBytes;
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await new Promise(r => server.close(r));
}
