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
  assert.deepEqual([...labErrors], []);
  assert.deepEqual(errors, []);
  console.log(`flow OK: ${checks.join(', ')}.`);
} finally {
  await browser.close();
  server.close();
}
