// Kortex Flow recipe on the compiled web app (dist/apps/apps.flow), served locally.
// Needs `cargo xtask build-app apps.flow --web`. CHROME_PATH may point at an installed Chrome.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { chromium } from 'playwright';

const dist = resolve(import.meta.dirname, '../../dist/apps/apps.flow');
assert.ok(existsSync(resolve(dist, 'index.html')), 'dist/apps/apps.flow missing: run `cargo xtask build-app apps.flow --web`');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };
const server = createServer((request, response) => {
  let path = resolve(dist, '.' + decodeURIComponent(new URL(request.url, 'http://x').pathname));
  if (path !== dist && !path.startsWith(dist + sep)) return response.writeHead(403).end();
  if (existsSync(path) && statSync(path).isDirectory()) path = resolve(path, 'index.html');
  if (!existsSync(path)) return response.writeHead(404).end();
  response.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' }).end(readFileSync(path));
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || undefined });
const checks = [];
const node = (page, id) => page.locator(`[data-node="${id}"]`);
const left = async (page, id) => parseFloat(await node(page, id).evaluate(e => e.style.left));
// Drags with real pointer events from one handle (or card header) to another point.
const dragTo = async (page, from, x, y) => {
  const box = await from.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(x, y, { steps: 8 });
  await page.mouse.up();
};
const center = async locator => {
  const b = await locator.boundingBox();
  return [b.x + b.width / 2, b.y + b.height / 2];
};
try {
  const context = await browser.newContext({ locale: 'fr-FR', acceptDownloads: true, viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.locator('.flow-empty').waitFor();
  assert.equal(await page.locator('[data-node]').count(), 0);
  checks.push('empty canvas');

  // Chain by clicking: each added tool lands to the right of the selected node and is linked to it.
  await page.locator('[data-add="text"]').click();
  await page.locator('.flow-inspector textarea').fill('  Hello   Kortex  ');
  await page.locator('[data-add="text.cleaner"]').click();
  await page.locator('[data-add="text.case-converter"]').click();
  assert.equal(await page.locator('[data-node]').count(), 3);
  assert.equal(await page.locator('.flow-link:not(.live)').count(), 2);
  assert.ok((await left(page, 'n1')) < (await left(page, 'n2')) && (await left(page, 'n2')) < (await left(page, 'n3')));
  await page.locator('.app-note[role="alert"]').waitFor({ state: 'detached' });
  checks.push('chain by click');

  // Running works in the browser through the embedded engines, with live state per node.
  await node(page, 'n3').locator('.flow-node-head').click();
  await page.locator('.flow-inspector select').filter({ hasText: 'snake' }).selectOption('snake');
  await page.getByRole('button', { name: /Exécuter/ }).click();
  await page.locator('[data-node="n3"][data-state="done"]').waitFor({ timeout: 20000 });
  assert.equal(await page.locator('[data-state="done"]').count(), 3);
  await node(page, 'n3').locator('.flow-node-head').click();
  await page.locator('.flow-result', { hasText: 'hello_kortex' }).waitFor();
  checks.push('run in browser');

  // Show everything (the canvas followed the last added card), then move a card with the pointer.
  await page.getByRole('button', { name: 'Tout voir' }).click();
  await page.waitForTimeout(150);
  const before = await left(page, 'n3');
  const [hx, hy] = await center(node(page, 'n3').locator('.flow-node-head'));
  await page.mouse.move(hx, hy);
  await page.mouse.down();
  await page.mouse.move(hx + 120, hy + 90, { steps: 6 });
  await page.mouse.up();
  assert.ok((await left(page, 'n3')) > before + 40, 'card moved');
  checks.push('drag a card');

  // Cut a link with its midpoint handle, then redraw it by dragging output -> input.
  await page.locator('[data-edge="n1.text>n2.text"]').hover();
  await page.locator('[data-cut="n1.text>n2.text"]').click({ force: true });
  assert.equal(await page.locator('.flow-link:not(.live)').count(), 1);
  await page.locator('.app-note[role="alert"]', { hasText: 'Entrée obligatoire non reliée' }).waitFor();
  const [ix, iy] = await center(page.locator('[data-port="n2.text"][data-side="in"]'));
  await dragTo(page, page.locator('[data-port="n1.text"][data-side="out"]'), ix, iy);
  assert.equal(await page.locator('[data-edge="n1.text>n2.text"]').count(), 1);
  checks.push('cut and drag-to-connect');

  // Refused connections leave the graph untouched: an output onto its own node's input.
  const [sx, sy] = await center(page.locator('[data-port="n2.text"][data-side="in"]'));
  await dragTo(page, page.locator('[data-port="n2.text"][data-side="out"]'), sx, sy);
  assert.equal(await page.locator('.flow-link:not(.live)').count(), 2);
  // A loop n3 -> n2 is refused too.
  await dragTo(page, page.locator('[data-port="n3.text"][data-side="out"]'), sx, sy);
  assert.equal(await page.locator('[data-edge="n3.text>n2.text"]').count(), 0);
  checks.push('refused connections');

  // Undo / redo with the keyboard.
  await node(page, 'n3').locator('.flow-node-head').click();
  await page.locator('#flow-canvas').focus();
  await page.keyboard.press('Delete');
  assert.equal(await page.locator('[data-node]').count(), 2);
  await page.keyboard.press('Control+z');
  assert.equal(await page.locator('[data-node]').count(), 3);
  await page.keyboard.press('Control+y');
  assert.equal(await page.locator('[data-node]').count(), 2);
  await page.keyboard.press('Control+z');
  assert.equal(await page.locator('[data-node]').count(), 3);
  checks.push('delete, undo, redo');

  // The wheel zooms and the minimap is present.
  const zoom = async () => parseInt(await page.locator('.flow-zoom').textContent(), 10);
  const z0 = await zoom();
  const [cx, cy] = await center(page.locator('#flow-canvas'));
  await page.mouse.move(cx, cy);
  await page.mouse.wheel(0, -300);
  await page.waitForFunction(z => parseInt(document.querySelector('.flow-zoom').textContent, 10) > z, z0);
  await page.getByRole("button", { name: "Tout voir" }).click();
  assert.equal(await page.locator('.flow-minimap').count(), 1);
  checks.push('zoom + minimap');

  // Export is versioned JSON describing the graph and its layout.
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('button', { name: 'Exporter (JSON)' }).click(),
  ]);
  const exported = JSON.parse(readFileSync(await download.path(), 'utf8'));
  assert.equal(exported.version, 1);
  assert.deepEqual(exported.nodes.map(n => [n.id, n.kind]), [['n1', 'text'], ['n2', 'tool'], ['n3', 'tool']]);
  assert.ok(exported.nodes.every(n => Number.isInteger(n.pos?.x) && Number.isInteger(n.pos?.y)));
  assert.equal(exported.nodes[2].params.case, 'snake');
  assert.equal(exported.edges.length, 2);
  checks.push('export with positions');

  // The workflow survives a reload; a fresh page can import it; bad files are refused.
  await page.reload({ waitUntil: 'networkidle' });
  await node(page, 'n3').waitFor();
  checks.push('persisted');
  const fresh = await context.newPage();
  await fresh.addInitScript(() => localStorage.clear());
  await fresh.goto(origin + '/', { waitUntil: 'networkidle' });
  await fresh.getByText('Importer un workflow (JSON)').click();
  // A file written before the canvas existed (no positions) is laid out automatically.
  const legacy = { ...exported, nodes: exported.nodes.map(({ pos, ...n }) => n) };
  await fresh.locator('.flow-import textarea').fill(JSON.stringify(legacy));
  await fresh.getByRole('button', { name: 'Importer', exact: true }).click();
  await node(fresh, 'n3').waitFor();
  assert.ok((await left(fresh, 'n1')) < (await left(fresh, 'n3')));
  await fresh.locator('.flow-import textarea').fill('{nope');
  await fresh.getByRole('button', { name: 'Importer', exact: true }).click();
  await fresh.locator('.app-note[role="status"]', { hasText: 'Fichier JSON invalide' }).waitFor();
  await fresh.locator('.flow-import textarea').fill(JSON.stringify({ ...exported, version: 99 }));
  await fresh.getByRole('button', { name: 'Importer', exact: true }).click();
  await fresh.locator('.app-note[role="status"]', { hasText: 'version plus récente' }).waitFor();
  checks.push('import (legacy layout) + rejections');

  // Built-in nodes, in a fresh page: drag from the palette, a condition with a skipped branch.
  const lab = await context.newPage();
  const labErrors = [];
  lab.on('pageerror', e => labErrors.push(e.message));
  await lab.addInitScript(() => localStorage.clear());
  await lab.goto(origin + '/', { waitUntil: 'networkidle' });
  const [kx, ky] = await center(lab.locator('#flow-canvas'));
  await dragTo(lab, lab.locator('[data-add="text"]'), kx - 300, ky - 120);
  assert.equal(await lab.locator('[data-node="n1"][data-kind="text"]').count(), 1);
  checks.push('drag a node from the palette');
  await lab.locator('.flow-inspector textarea').fill('error: disk full');
  await lab.locator('[data-add="logic.condition"]').click();
  await lab.locator('.flow-inspector [data-field="value"]').fill('error');
  await lab.locator('[data-add="text.case-converter"]').click();
  await node(lab, 'n2').locator('.flow-node-head').click();
  await lab.locator('[data-add="text.slug-generator"]').click();
  assert.equal(await lab.locator('[data-edge="n2.then>n4.text"]').count(), 1);
  // Move that last link to the "else" branch: cut it, then draw it from n2.else.
  await lab.getByRole('button', { name: 'Tout voir' }).click();
  await lab.waitForTimeout(150);
  await lab.locator('[data-edge="n2.then>n4.text"]').hover();
  await lab.locator('[data-cut="n2.then>n4.text"]').click({ force: true });
  const [ex, ey] = await center(lab.locator('[data-port="n4.text"][data-side="in"]'));
  await dragTo(lab, lab.locator('[data-port="n2.else"][data-side="out"]'), ex, ey);
  assert.equal(await lab.locator('[data-edge="n2.else>n4.text"]').count(), 1);
  await lab.getByRole('button', { name: /Exécuter/ }).click();
  await lab.locator('[data-node="n3"][data-state="done"]').waitFor({ timeout: 20000 });
  await lab.locator('[data-node="n4"][data-state="skipped"]').waitFor();
  checks.push('condition runs one branch and skips the other');

  // Selection: shift-click two cards, copy, paste, then delete the copies and undo.
  await node(lab, 'n1').locator('.flow-node-head').click();
  await node(lab, 'n2').locator('.flow-node-head').click({ modifiers: ['Shift'] });
  assert.equal(await lab.locator('[data-selected="true"]').count(), 2);
  await lab.locator('#flow-canvas').focus();
  await lab.keyboard.press('Control+c');
  await lab.keyboard.press('Control+v');
  assert.equal(await lab.locator('[data-node]').count(), 6);
  assert.equal(await lab.locator('.flow-link').count(), 4, 'the link between the copies came along');
  await lab.keyboard.press('Delete');
  assert.equal(await lab.locator('[data-node]').count(), 4);
  await lab.keyboard.press('Control+z');
  assert.equal(await lab.locator('[data-node]').count(), 6);
  checks.push('multi-select, copy/paste, delete, undo');

  // The minimap moves the view when clicked.
  const viewBefore = await lab.locator('.flow-layer').getAttribute('style');
  const mini = await lab.locator('#flow-minimap').boundingBox();
  await lab.mouse.click(mini.x + 8, mini.y + 8);
  await lab.waitForFunction(b => document.querySelector('.flow-layer').getAttribute('style') !== b, viewBefore);
  checks.push('clickable minimap');

  // A file node: the bytes come from the user's disk and reach a real tool engine.
  const files = await context.newPage();
  await files.addInitScript(() => localStorage.clear());
  await files.goto(origin + '/', { waitUntil: 'networkidle' });
  await files.locator('[data-add="file"]').click();
  await files.getByRole('button', { name: /Exécuter/ }).isDisabled();
  await files.locator('[data-add="secure.hash-calculator"]').click();
  assert.equal(await files.locator('[data-edge="n1.file>n2.file"]').count(), 1, 'file output chained to the file input');
  assert.ok(await files.getByRole('button', { name: /Exécuter/ }).isDisabled(), 'no file chosen yet');
  await files.locator('[data-node="n1"] .flow-node-head').click();
  await files.locator('.flow-inspector [data-field="file"]').setInputFiles({ name: 'abc.txt', mimeType: 'text/plain', buffer: Buffer.from('abc') });
  await files.locator('[data-field="file-status"]', { hasText: 'abc.txt' }).waitFor();
  await files.getByRole('button', { name: /Exécuter/ }).click();
  await files.locator('[data-node="n2"][data-state="done"]').waitFor({ timeout: 20000 });
  await files.locator('[data-node="n2"] .flow-node-head').click();
  await files.locator('.flow-result', { hasText: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad' }).waitFor();
  // The workflow JSON never carries the bytes.
  const saved = await files.evaluate(() => Object.values(localStorage).join('\n'));
  assert.ok(!saved.includes('YWJj') && !saved.includes('abc.txt'));
  checks.push('file node reaches a real engine, bytes stay out of storage');
  // The delay node waits on the page's timer, then passes the text on.
  const wait = await context.newPage();
  await wait.addInitScript(() => localStorage.clear());
  await wait.goto(origin + '/', { waitUntil: 'networkidle' });
  await wait.locator('[data-add="text"]').click();
  await wait.locator('.flow-inspector textarea').fill('later');
  await wait.locator('[data-add="logic.delay"]').click();
  await wait.locator('.flow-inspector [data-field="ms"]').fill('300');
  const started = Date.now();
  await wait.getByRole('button', { name: /Exécuter/ }).click();
  await wait.locator('[data-node="n2"][data-state="done"]').waitFor({ timeout: 20000 });
  assert.ok(Date.now() - started >= 250, 'the delay really waited');
  await wait.locator('[data-node="n2"] .flow-node-head').click();
  await wait.locator('.flow-result', { hasText: 'later' }).waitFor();
  checks.push('delay node');
  // Bounded expressions and a loop over lines.
  const calc = await context.newPage();
  await calc.addInitScript(() => localStorage.clear());
  await calc.goto(origin + '/', { waitUntil: 'networkidle' });
  await calc.locator('[data-add="text"]').click();
  await calc.locator('.flow-inspector textarea').fill('  ada  ');
  await calc.locator('[data-add="logic.template"]').click();
  await calc.locator('.flow-inspector [data-field="template"]').fill('Hello {a|trim|upper}!');
  await calc.getByRole('button', { name: /Exécuter/ }).click();
  await calc.locator('[data-node="n2"][data-state="done"]').waitFor({ timeout: 20000 });
  await calc.locator('[data-node="n2"] .flow-node-head').click();
  await calc.locator('.flow-result', { hasText: 'Hello ADA!' }).waitFor();
  checks.push('template node');
  await calc.locator('[data-node="n1"] .flow-node-head').click();
  await calc.locator('.flow-inspector textarea').fill('alpha\nbeta');
  await calc.locator('[data-add="logic.foreach"]').click();
  assert.equal(await calc.locator('[data-edge="n1.text>n3.text"]').count(), 1);
  await calc.locator('.flow-inspector [data-field="tool"]').selectOption('text.case-converter');
  await calc.locator('.flow-inspector [data-field="options"]').fill('case=upper');
  await calc.getByRole('button', { name: /Exécuter/ }).click();
  await calc.locator('[data-node="n3"][data-state="done"]').waitFor({ timeout: 30000 });
  await calc.locator('[data-node="n3"] .flow-node-head').click();
  await calc.locator('.flow-result', { hasText: 'ALPHA' }).waitFor();
  assert.ok((await calc.locator('.flow-result').first().textContent()).includes('BETA'));
  checks.push('for-each-line loop runs a real engine per line');
  // The HTTP node goes through the account API's proxy in the browser; secrets stay on the device.
  const net = await context.newPage();
  await net.addInitScript(() => localStorage.clear());
  const proxied = [];
  let answer = { status: 200, json: { status: 201, headers: {}, url: 'https://api.github.com/x', body_base64: Buffer.from('created').toString('base64') } };
  await net.route('https://api.kortex.site/v1/proxy', async route => {
    const request = route.request();
    const cors = { 'access-control-allow-origin': request.headers().origin ?? '*', 'access-control-allow-credentials': 'true', 'access-control-allow-headers': 'content-type, x-kortex-request, x-kortex-user', 'access-control-allow-methods': 'POST' };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers: cors });
    proxied.push({ body: JSON.parse(request.postData()), headers: request.headers() });
    await route.fulfill({ status: answer.status, headers: { ...cors, 'content-type': 'application/json' }, body: JSON.stringify(answer.json) });
  });
  await net.goto(origin + '/', { waitUntil: 'networkidle' });
  await net.locator('[data-panel="secrets"] summary').click();
  await net.locator('[data-field="secret-name"]').fill('TOKEN');
  await net.locator('[data-field="secret-value"]').fill('abc123');
  await net.locator('[data-action="save-secret"]').click();
  await net.locator('[data-panel="secrets"] code', { hasText: '{secret:TOKEN}' }).waitFor();
  assert.equal(await net.locator('[data-field="secret-value"]').inputValue(), '', 'the value is cleared after saving');
  await net.locator('[data-add="text"]').click();
  await net.locator('.flow-inspector textarea').fill('ping');
  await net.locator('[data-add="logic.http"]').click();
  await net.locator('.flow-inspector [data-field="method"]').selectOption('POST');
  await net.locator('.flow-inspector [data-field="url"]').fill('https://api.github.com/x/{secret:TOKEN}');
  await net.locator('.flow-inspector [data-field="headers"]').fill('Authorization: Bearer {secret:TOKEN}');
  await net.getByRole('button', { name: /Exécuter/ }).click();
  await net.locator('[data-node="n2"][data-state="done"]').waitFor({ timeout: 20000 });
  assert.equal(proxied.length, 1);
  assert.deepEqual(proxied[0].body, { url: 'https://api.github.com/x/abc123', method: 'POST', headers: { authorization: 'Bearer abc123' }, body: 'ping' });
  assert.equal(proxied[0].headers['x-kortex-request'], '1');
  await net.locator('[data-node="n2"] .flow-node-head').click();
  await net.locator('.flow-result', { hasText: 'created' }).waitFor();
  assert.ok((await net.locator('.flow-inspector').textContent()).includes('201'));
  checks.push('HTTP node through the proxy with a device secret');
  // The workflow (export and storage) holds the placeholder, never the secret value.
  const stored = await net.evaluate(() => Object.values(localStorage).find(v => v.includes('"nodes"')) ?? '');
  assert.ok(stored.includes('{secret:TOKEN}') && !stored.includes('abc123'));
  // A proxy refusal is explained, not shown as a code.
  answer = { status: 401, json: { error: 'unauthenticated' } };
  await net.getByRole('button', { name: /Exécuter/ }).click();
  await net.locator('[data-node="n2"][data-state="failed"]').waitFor({ timeout: 20000 });
  await net.locator('.flow-log', { hasText: 'Connectez-vous' }).waitFor();
  checks.push('proxy errors are translated');
  // A URL that breaks the policy is caught by validation before any request.
  await net.locator('[data-node="n2"] .flow-node-head').click();
  await net.locator('.flow-inspector [data-field="url"]').fill('https://127.0.0.1/admin');
  await net.locator('.app-note[role="alert"]', { hasText: 'url' }).waitFor();
  assert.ok(await net.getByRole('button', { name: /Exécuter/ }).isDisabled());
  assert.equal(proxied.length, 2, 'nothing was sent for the refused URL');
  checks.push('private URL refused before running');
  // A connector action: Telegram needs the token as a device secret and a typed chat id; the
  // previous node's text becomes the message.
  const tg = await context.newPage();
  await tg.addInitScript(() => localStorage.clear());
  const sent = [];
  await tg.route('https://api.kortex.site/v1/proxy', async route => {
    const request = route.request();
    const cors = { 'access-control-allow-origin': request.headers().origin ?? '*', 'access-control-allow-credentials': 'true', 'access-control-allow-headers': 'content-type, x-kortex-request, x-kortex-user', 'access-control-allow-methods': 'POST' };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers: cors });
    sent.push(JSON.parse(request.postData()));
    await route.fulfill({ status: 200, headers: { ...cors, 'content-type': 'application/json' }, body: JSON.stringify({ status: 200, headers: {}, url: 'x', body_base64: Buffer.from('{"ok":true}').toString('base64') }) });
  });
  await tg.goto(origin + '/', { waitUntil: 'networkidle' });
  await tg.locator('[data-panel="secrets"] summary').click();
  await tg.locator('[data-field="secret-name"]').fill('TELEGRAM_TOKEN');
  await tg.locator('[data-field="secret-value"]').fill('123:ABC');
  await tg.locator('[data-action="save-secret"]').click();
  await tg.locator('[data-add="text"]').click();
  await tg.locator('.flow-inspector textarea').fill('Build "ok"\nall green');
  await tg.locator('[data-add="connector.telegram.send_message"]').click();
  assert.equal(await tg.locator('[data-edge="n1.text>n2.chat_id"]').count(), 1, 'the first free input receives the text');
  await tg.locator('[data-connector-help]').waitFor();
  // Rewire: the text feeds the message, the chat id is typed in.
  await tg.locator('[data-edge="n1.text>n2.chat_id"]').hover();
  await tg.locator('[data-cut="n1.text>n2.chat_id"]').click({ force: true });
  await tg.getByRole('button', { name: 'Tout voir' }).click();
  await tg.waitForTimeout(150);
  const [tx, ty] = await center(tg.locator('[data-port="n2.text"][data-side="in"]'));
  await dragTo(tg, tg.locator('[data-port="n1.text"][data-side="out"]'), tx, ty);
  await tg.locator('[data-node="n2"] .flow-node-head').click();
  await tg.locator('.flow-inspector [data-field="chat_id"]').fill('42');
  await tg.getByRole('button', { name: /Exécuter/ }).click();
  await tg.locator('[data-node="n2"][data-state="done"]').waitFor({ timeout: 20000 });
  assert.equal(sent.length, 1);
  assert.equal(sent[0].url, 'https://api.telegram.org/bot123:ABC/sendMessage');
  assert.equal(sent[0].method, 'POST');
  assert.deepEqual(JSON.parse(sent[0].body), { chat_id: '42', text: 'Build "ok"\nall green' });
  checks.push('Telegram connector builds its request from node text and a device secret');
  // The Agent node: consent first, then the model (simulated gateway) with text, table, JSON repair
  // and local tools. The wire format is the gateway's real SSE.
  const brain = await context.newPage();
  await brain.addInitScript(() => localStorage.clear());
  const asked = [];
  let script = [];
  let signedIn = true;
  const sse = (content, toolCall) => {
    const delta = toolCall
      ? { tool_calls: [{ index: 0, id: 'model-call', function: { name: toolCall.name, arguments: JSON.stringify(toolCall.args) } }] }
      : { content };
    return `data: ${JSON.stringify({ delta, finish_reason: null })}\n\ndata: ${JSON.stringify({ delta: {}, finish_reason: toolCall ? 'tool_calls' : 'stop' })}\n\nevent: done\ndata: ${JSON.stringify({ credits: { percent: 80, status: 'ok', resets_in_seconds: null } })}\n\n`;
  };
  await brain.route('https://api.kortex.site/**', async route => {
    const request = route.request();
    const cors = { 'access-control-allow-origin': request.headers().origin ?? '*', 'access-control-allow-credentials': 'true', 'access-control-allow-headers': 'content-type, x-kortex-request, x-kortex-user', 'access-control-allow-methods': 'GET, POST' };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers: cors });
    const path = new URL(request.url()).pathname;
    if (path === '/v1/me') {
      return signedIn
        ? route.fulfill({ status: 200, headers: { ...cors, 'content-type': 'application/json' }, body: JSON.stringify({ id: 'u1', email: 'tester@example.test', created_at: 1 }) })
        : route.fulfill({ status: 401, headers: { ...cors, 'content-type': 'application/json' }, body: '{"error":"unauthenticated"}' });
    }
    if (path === '/v1/ai/credits') return route.fulfill({ status: 200, headers: { ...cors, 'content-type': 'application/json' }, body: JSON.stringify({ percent: 80, status: 'ok', resets_in_seconds: null }) });
    if (path === '/v1/ai/chat') {
      asked.push(JSON.parse(request.postData()));
      const next = script.shift() ?? { content: 'no script' };
      return route.fulfill({ status: 200, headers: { ...cors, 'content-type': 'text/event-stream' }, body: sse(next.content, next.tool) });
    }
    return route.fulfill({ status: 404, headers: cors, body: '{}' });
  });
  await brain.goto(origin + '/', { waitUntil: 'networkidle' });
  await brain.locator('[data-add="text"]').click();
  await brain.locator('.flow-inspector textarea').fill('Kortex builds local-first tools. Ada Lovelace (36) wrote the first program.');
  await brain.locator('[data-add="logic.ai"]').click();
  await brain.locator('[data-panel="ai"]').waitFor();
  await brain.locator('[data-ai-account="ready"]', { hasText: 'tester@example.test' }).waitFor();
  await brain.locator('[data-ai-credits]', { hasText: '80%' }).waitFor();
  // No consent, no run: nothing may leave the device.
  assert.ok(await brain.getByRole('button', { name: /Exécuter/ }).isDisabled());
  await brain.locator('[data-field="ai-consent"]').check();
  await brain.locator('.flow-inspector [data-field="task"]').selectOption('summarize');
  script = [{ content: 'Kortex: local-first tools.' }];
  await brain.getByRole('button', { name: /Exécuter/ }).click();
  await brain.locator('[data-node="n2"][data-state="done"]').waitFor({ timeout: 20000 });
  assert.equal(asked.length, 1);
  const first = asked[0];
  assert.equal(first.consent, true);
  assert.equal(first.stream, true);
  assert.ok(/DATA|DONNÉE/.test(first.messages[0].content), 'the system prompt marks the source as data');
  assert.ok(first.messages[1].content.includes('<source>\nKortex builds local-first tools.'));
  assert.ok(first.messages[1].content.includes('Summarize the source') || first.messages[1].content.includes('Résume la source'));
  await brain.locator('[data-node="n2"] .flow-node-head').click();
  await brain.locator('.flow-result', { hasText: 'Kortex: local-first tools.' }).waitFor();
  checks.push('Agent node: consent, account, credits, prompt sent as data');

  // A table: columns from the fields, filled from the model's JSON.
  await brain.locator('.flow-inspector [data-field="task"]').selectOption('extract');
  await brain.locator('.flow-inspector [data-field="format"]').selectOption('table');
  await brain.locator('.flow-inspector [data-field="fields"]').fill('name\nage');
  script = [{ content: '```json\n[{"name":"Ada Lovelace","age":36}]\n```' }];
  await brain.getByRole('button', { name: /Exécuter/ }).click();
  await brain.locator('[data-node="n2"][data-state="done"]').waitFor({ timeout: 20000 });
  await brain.locator('.flow-result', { hasText: '1 × 2' }).waitFor();
  assert.ok((await brain.locator('.flow-inspector').textContent()).includes('name,age'));
  checks.push('Agent node: table output');

  // Invalid JSON is repaired once.
  await brain.locator('.flow-inspector [data-field="format"]').selectOption('json');
  const askedBefore = asked.length;
  script = [{ content: 'The person is Ada.' }, { content: '{"name":"Ada Lovelace"}' }];
  await brain.getByRole('button', { name: /Exécuter/ }).click();
  await brain.locator('[data-node="n2"][data-state="done"]').waitFor({ timeout: 20000 });
  assert.equal(asked.length - askedBefore, 2, 'one question and one repair');
  assert.equal(asked.at(-1).messages.length, 4);
  checks.push('Agent node: JSON repaired once');

  // Tools: the model names the data by reference, the engine runs locally, the answer expands.
  await brain.locator('.flow-inspector [data-field="format"]').selectOption('text');
  await brain.locator('.flow-inspector [data-field="task"]').selectOption('prompt');
  await brain.locator('[data-node="n1"] .flow-node-head').click();
  await brain.locator('.flow-inspector textarea').fill('  Hello   Kortex  ');
  await brain.locator('[data-node="n2"] .flow-node-head').click();
  await brain.locator('[data-panel="agent-tools"] summary').click();
  await brain.locator('[data-agent-tool="text.cleaner"]').check();
  await brain.locator('.flow-inspector [data-field="prompt"]').fill('Clean the source and reply with the result');
  const base = asked.length;
  script = [{ tool: { name: 'text__cleaner', args: { text: '@texte1' } } }, { content: 'Result: @c1.text' }];
  await brain.getByRole('button', { name: /Exécuter/ }).click();
  await brain.locator('[data-node="n2"][data-state="done"]').waitFor({ timeout: 30000 });
  assert.equal(asked.length - base, 2);
  assert.equal(asked[base].tools.length, 1);
  assert.ok(!JSON.stringify(asked[base + 1]).includes('Hello   Kortex'), 'the model never saw the source text');
  assert.ok(JSON.stringify(asked[base + 1]).includes('@c1.text'), 'it only got a summary with references');
  await brain.locator('[data-node="n2"] .flow-node-head').click();
  await brain.locator('.flow-result', { hasText: 'Result: Hello Kortex' }).waitFor();
  checks.push('Agent node: local tools by reference, references expanded in the answer');

  // Signed out: the panel says so and the run stays closed.
  signedIn = false;
  const guest = await context.newPage();
  await guest.addInitScript(() => localStorage.clear());
  await guest.route('https://api.kortex.site/**', async route => {
    const request = route.request();
    const cors = { 'access-control-allow-origin': request.headers().origin ?? '*', 'access-control-allow-credentials': 'true', 'access-control-allow-headers': 'content-type, x-kortex-request, x-kortex-user', 'access-control-allow-methods': 'GET, POST' };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers: cors });
    return route.fulfill({ status: 401, headers: { ...cors, 'content-type': 'application/json' }, body: '{"error":"unauthenticated"}' });
  });
  await guest.goto(origin + '/', { waitUntil: 'networkidle' });
  await guest.locator('[data-add="logic.ai"]').click();
  await guest.locator('[data-ai-account="signed-out"]').waitFor();
  await guest.locator('[data-field="ai-consent"]').check();
  assert.ok(await guest.getByRole('button', { name: /Exécuter/ }).isDisabled());
  checks.push('Agent node: signed-out account blocks the run');
  assert.deepEqual([...labErrors], []);
  assert.deepEqual(errors, []);
  console.log(`flow OK: ${checks.join(', ')}.`);
} finally {
  await browser.close();
  server.close();
}
