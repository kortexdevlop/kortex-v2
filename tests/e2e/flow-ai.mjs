// Flow's Agent node end to end with the REAL model: the compiled web app (dist/apps/apps.flow) in a
// browser, the real api Worker running locally and Gemma on Workers AI behind it. It spends a few
// neurons of the Cloudflare account's free quota. A throwaway account is registered on the LOCAL api
// only (never printed, never production).
//
//   cargo xtask build-app apps.flow --web
//   wrangler dev --port 8787 --var ALLOWED_ORIGINS:http://127.0.0.1:8790   (in services/api)
//   FLOW_E2E_API=http://127.0.0.1:8787 node tests/e2e/flow-ai.mjs
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { randomBytes } from 'node:crypto';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { chromium } from 'playwright';
import { deriveAuthKey } from '../../services/shared/src/auth.ts';

const api = process.env.FLOW_E2E_API ?? 'http://127.0.0.1:8787';
assert.ok(['localhost', '127.0.0.1'].includes(new URL(api).hostname), 'this recipe only talks to a local api');
const dist = resolve(import.meta.dirname, '../../dist/apps/apps.flow');
assert.ok(existsSync(resolve(dist, 'index.html')), 'dist/apps/apps.flow missing: run `cargo xtask build-app apps.flow --web`');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };
const server = createServer((request, response) => {
  let path = resolve(dist, '.' + decodeURIComponent(new URL(request.url, 'http://x').pathname));
  if (path !== dist && !path.startsWith(dist + sep)) return response.writeHead(403).end();
  if (existsSync(path) && statSync(path).isDirectory()) path = resolve(path, 'index.html');
  if (!existsSync(path)) return response.writeHead(404).end();
  response.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' }).end(readFileSync(path));
});
await new Promise(done => server.listen(8790, '127.0.0.1', done));
const origin = 'http://127.0.0.1:8790';

const email = `flow-e2e-${randomBytes(4).toString('hex')}@example.test`;
const password = randomBytes(18).toString('base64url');
const registered = await fetch(`${api}/v1/auth/register`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json', Origin: origin },
  body: JSON.stringify({ email, auth_key: await deriveAuthKey(email, password, 5000), kdf: 1 }),
});
assert.equal(registered.status, 201, 'local account creation (is the api running with this origin allowed?)');
const [cookieName, ...cookieValue] = (registered.headers.get('set-cookie') ?? '').split(';')[0].split('=');

const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || undefined });
const checks = [];
try {
  const context = await browser.newContext({ locale: 'fr-FR' });
  await context.addCookies([{ name: cookieName, value: cookieValue.join('='), domain: '127.0.0.1', path: '/' }]);
  const page = await context.newPage();
  const problems = [];
  page.on('pageerror', e => problems.push(e.message));
  await page.goto(`${origin}/?api=${encodeURIComponent(api)}`, { waitUntil: 'networkidle' });
  const result = () => page.locator('.flow-result').first().textContent();
  const run = async () => {
    await page.getByRole('button', { name: /Exécuter/ }).click();
    await page.locator('[data-node="n2"][data-state="done"], [data-node="n2"][data-state="failed"]').waitFor({ timeout: 120000 });
    const state = await page.locator('[data-node="n2"]').getAttribute('data-state');
    if (state !== 'done') throw new Error('agent failed: ' + (await page.locator('.flow-log').innerText().catch(() => '')));
    await page.locator('[data-node="n2"] .flow-node-head').click();
  };
  const toSource = async () => {
    await page.getByRole('button', { name: 'Tout voir' }).click();
    await page.waitForTimeout(200);
    await page.locator('[data-node="n1"] .flow-node-head').click();
  };
  const field = id => page.locator(`.flow-inspector [data-field="${id}"]`);

  await page.locator('[data-add="text"]').click();
  await page.locator('.flow-inspector textarea').fill('Kortex est une suite d’outils qui fonctionnent sur votre appareil. Ada Lovelace, née en 1815, a écrit le premier programme informatique.');
  await page.locator('[data-add="logic.ai"]').click();
  await page.locator('[data-ai-account="ready"]', { hasText: email }).waitFor();
  await page.locator('[data-ai-credits]').waitFor();
  await page.locator('[data-field="ai-consent"]').check();
  checks.push('account and credits from the real api');

  await field('task').selectOption('summarize');
  await run();
  assert.ok((await result()).trim().length > 20, 'a summary came back');
  checks.push('summary');

  await field('task').selectOption('extract');
  await field('format').selectOption('json');
  await field('fields').fill('nom\nannee_de_naissance');
  await run();
  const json = JSON.parse(await result());
  assert.match(String(json.nom), /Lovelace/);
  assert.match(String(json.annee_de_naissance), /1815/);
  checks.push('structured JSON extraction');

  await field('format').selectOption('table');
  await run();
  assert.ok((await page.locator('.flow-inspector').textContent()).includes('nom,annee_de_naissance') || (await page.locator('.flow-inspector').textContent()).includes('nom'), 'a table with the requested columns');
  checks.push('table output');

  await toSource();
  await page.locator('.flow-inspector textarea').fill('Ce produit est génial, je suis ravi !');
  await page.locator('[data-node="n2"] .flow-node-head').click();
  await field('task').selectOption('classify');
  await field('format').selectOption('text');
  await field('fields').fill('positif\nnégatif\nneutre');
  await run();
  assert.match((await result()).toLowerCase(), /positif/);
  checks.push('classification');

  // Local tools: the model sees a reference, the engine runs on this device.
  await toSource();
  await page.locator('.flow-inspector textarea').fill('  Hello   Kortex  ');
  await page.locator('[data-node="n2"] .flow-node-head').click();
  await field('task').selectOption('prompt');
  await field('fields').fill('').catch(() => {});
  await field('prompt').fill('Nettoie la source avec l’outil de nettoyage de texte, puis réponds uniquement avec le résultat.');
  await page.locator('[data-panel="agent-tools"] summary').click();
  await page.locator('[data-agent-tool="text.cleaner"]').check();
  await run();
  assert.match(await result(), /Hello Kortex/);
  checks.push('agent called a local tool by reference');
  assert.deepEqual(problems, []);
  console.log(`flow AI OK (real Gemma): ${checks.join(', ')}.`);
} finally {
  await browser.close();
  server.close();
}
