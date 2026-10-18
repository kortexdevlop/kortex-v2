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
  await page.locator('[data-tool="text.cleaner"]').click();
  await page.locator('[data-tool="text.case-converter"]').click();
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
  assert.deepEqual(errors, []);
  console.log(`flow OK: ${checks.join(', ')}.`);
} finally {
  await browser.close();
  server.close();
}
