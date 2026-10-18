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
// Steps are listed in execution order: address them by id, not by position.
const step = (page, id) => page.locator('.flow-step', { hasText: `${id} ·` });
try {
  const context = await browser.newContext({ locale: 'fr-FR', acceptDownloads: true });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  await page.goto(origin + '/', { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Ajouter un texte' }).waitFor();
  assert.equal(await page.locator('.flow-step').count(), 0);
  checks.push('empty state');

  // Build text -> cleaner -> case converter with explicit wiring.
  await page.getByRole('button', { name: 'Ajouter un texte' }).click();
  await page.locator('.flow-step textarea').first().fill('  Hello   Kortex  ');
  const toolSelect = page.locator('.flow-bar select').first();
  await toolSelect.selectOption({ label: 'Nettoyeur de texte' });
  await page.getByRole('button', { name: 'Ajouter l’outil' }).click();
  assert.equal(await page.locator('.flow-step').count(), 2);
  // The required input is not wired yet: Flow says so and does not offer to run.
  await page.locator('.app-note[role="alert"]', { hasText: 'Entrée obligatoire non reliée' }).waitFor();
  assert.ok(await page.getByRole('button', { name: 'Exécuter' }).isDisabled());
  checks.push('missing input reported');
  await step(page, 'n2').locator('select').filter({ hasText: 'n1.text' }).selectOption('n1.text');
  await page.locator('.app-note[role="alert"]').waitFor({ state: 'detached' });
  checks.push('wiring clears the issue');

  await toolSelect.selectOption({ label: 'Convertisseur de casse' });
  await page.getByRole('button', { name: 'Ajouter l’outil' }).click();
  await step(page, 'n3').locator('select').filter({ hasText: 'n2.text' }).selectOption('n2.text');
  await step(page, 'n3').locator('select').filter({ hasText: 'snake' }).selectOption('snake');
  assert.equal(await page.locator('.flow-step').count(), 3);
  // Running needs the desktop app: stated, never faked.
  await page.locator('.app-note', { hasText: 'application bureau' }).waitFor();
  assert.ok(await page.getByRole('button', { name: 'Exécuter' }).isDisabled());
  checks.push('web run honestly unavailable');

  // Export is versioned JSON describing the graph.
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('button', { name: 'Exporter (JSON)' }).click(),
  ]);
  const exported = JSON.parse(readFileSync(await download.path(), 'utf8'));
  assert.equal(exported.version, 1);
  assert.deepEqual(exported.nodes.map(n => [n.id, n.kind]), [['n1', 'text'], ['n2', 'tool'], ['n3', 'tool']]);
  assert.equal(exported.nodes[2].tool, 'text.case-converter');
  assert.equal(exported.nodes[2].params.case, 'snake');
  assert.equal(exported.edges.length, 2);
  checks.push('export');

  // The workflow survives a reload (local storage) and a fresh page can import it.
  await page.reload({ waitUntil: 'networkidle' });
  await page.locator('.flow-step').nth(2).waitFor();
  checks.push('persisted');
  const fresh = await context.newPage();
  await fresh.addInitScript(() => localStorage.clear());
  await fresh.goto(origin + '/', { waitUntil: 'networkidle' });
  await fresh.getByText('Importer un workflow (JSON)').click();
  await fresh.locator('.flow-import textarea').fill(JSON.stringify(exported));
  await fresh.getByRole('button', { name: 'Importer', exact: true }).click();
  await fresh.locator('.flow-step').nth(2).waitFor();
  // Newer or invalid files are refused with a clear message.
  await fresh.locator('.flow-import textarea').fill('{nope');
  await fresh.getByRole('button', { name: 'Importer', exact: true }).click();
  await fresh.locator('.app-note[role="status"]', { hasText: 'Fichier JSON invalide' }).waitFor();
  await fresh.locator('.flow-import textarea').fill(JSON.stringify({ ...exported, version: 99 }));
  await fresh.getByRole('button', { name: 'Importer', exact: true }).click();
  await fresh.locator('.app-note[role="status"]', { hasText: 'version plus récente' }).waitFor();
  checks.push('import + rejections');

  // A loop is detected.
  await step(fresh, 'n2').locator('select').filter({ hasText: 'n3.text' }).selectOption('n3.text');
  await fresh.locator('.app-note[role="alert"]', { hasText: 'boucle' }).waitFor();
  checks.push('cycle detected');
  assert.deepEqual(errors, []);
  console.log(`flow OK: ${checks.join(', ')}.`);
} finally {
  await browser.close();
  server.close();
}
