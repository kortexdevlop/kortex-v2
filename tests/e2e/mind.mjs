// Mind end to end, with the REAL model: the compiled web app (dist/apps/apps.mind) in a browser,
// the real api Worker running locally (`pnpm --filter @kortex/api dev` with the page origin in
// ALLOWED_ORIGINS) and Gemma on Workers AI behind it. It spends a few neurons of the Cloudflare
// account's free quota. A throwaway account is registered on the LOCAL api only.
//
//   cargo xtask build-app apps.mind --web
//   wrangler dev --port 8787 --var ALLOWED_ORIGINS:http://127.0.0.1:8790   (in services/api)
//   MIND_E2E_API=http://127.0.0.1:8787 node tests/e2e/mind.mjs
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { randomBytes } from 'node:crypto';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { chromium } from 'playwright';
import { deriveAuthKey } from '../../services/shared/src/auth.ts';

const api = process.env.MIND_E2E_API ?? 'http://127.0.0.1:8787';
assert.ok(['localhost', '127.0.0.1'].includes(new URL(api).hostname), 'this recipe only talks to a local api');
const dist = resolve(import.meta.dirname, '../../dist/apps/apps.mind');
assert.ok(existsSync(resolve(dist, 'index.html')), 'dist/apps/apps.mind missing: run `cargo xtask build-app apps.mind --web`');
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

// A throwaway local account (random credentials, never printed).
const email = `mind-e2e-${randomBytes(4).toString('hex')}@example.test`;
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
  const context = await browser.newContext({ locale: 'fr-FR', acceptDownloads: true });
  await context.addCookies([{ name: cookieName, value: cookieValue.join('='), domain: '127.0.0.1', path: '/' }]);
  const page = await context.newPage();
  const problems = [];
  page.on('pageerror', e => problems.push(e.message));
  const gatewayBodies = [];
  page.on('request', r => { if (r.url().endsWith('/v1/ai/chat') && r.method() === 'POST') gatewayBodies.push(r.postData() ?? ''); });
  await page.goto(`${origin}/?api=${encodeURIComponent(api)}`, { waitUntil: 'networkidle' });

  // 1. Account and credits are read from the real api.
  await page.locator('.mind-account .app-badge', { hasText: email }).waitFor();
  await page.locator('.mind-meter').waitFor();
  assert.equal(await page.locator('.mind-meter').getAttribute('aria-valuenow'), '100');
  checks.push('signed in, credits shown');

  // 2. Sending needs the explicit consent first.
  const message = page.getByLabel('Votre message');
  await message.fill('Bonjour');
  assert.ok(await page.getByRole('button', { name: 'Envoyer' }).isDisabled(), 'no consent, no sending');
  await page.getByRole('button', { name: 'J’ai compris, continuer' }).click();
  checks.push('consent gate');

  // 3. A real request: the quoted text must never leave the device, only a reference does.
  await message.fill("Nettoie le texte '  bonjour    le   monde  ' puis mets le résultat en majuscules.");
  await page.getByRole('button', { name: 'Envoyer' }).click();
  const approve = page.getByRole('button', { name: 'Valider et exécuter' });
  for (let round = 1; round <= 4; round++) {
    await approve.waitFor({ timeout: 90000 });
    assert.equal(await page.locator('.mind-result').count(), round - 1, 'nothing runs before approval');
    await approve.click();
    await page.locator('.mind-result').nth(round - 1).waitFor({ timeout: 60000 });
    if (await page.locator('.mind-output pre', { hasText: /^BONJOUR LE MONDE$/ }).count()) break;
  }
  await page.locator('.mind-output pre', { hasText: /^BONJOUR LE MONDE$/ }).waitFor();
  await page.locator('.mind-assistant').last().waitFor({ timeout: 90000 });
  checks.push('plan approved and run locally by real engines');
  const sent = gatewayBodies.join('\n');
  assert.ok(gatewayBodies.length >= 3, `at least plan, plan, answer: ${gatewayBodies.length} requests`);
  assert.ok(sent.includes('@texte1'), 'the model works on a reference');
  assert.ok(!sent.includes('bonjour    le   monde'), 'the quoted text never left the device');
  assert.ok(!sent.includes('BONJOUR LE MONDE') && !sent.includes('bonjour le monde'), 'results go back as references, not data');
  checks.push('no user data sent to the model');

  // 4. The executed plan exports as a Flow workflow.
  const [download] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', { name: 'Exporter en workflow Flow' }).click()]);
  const workflow = JSON.parse(readFileSync(await download.path(), 'utf8'));
  assert.equal(workflow.version, 1);
  assert.ok(workflow.nodes.some(n => n.kind === 'text' && n.value === '  bonjour    le   monde  '));
  assert.ok(workflow.nodes.filter(n => n.kind === 'tool').length >= 2);
  assert.ok(workflow.edges.length >= 2);
  checks.push('exported as a Flow workflow');

  // 5. The conversation survives a reload.
  await page.reload({ waitUntil: 'networkidle' });
  await page.locator('.mind-user').first().waitFor();
  assert.ok((await page.locator('.mind-user').first().textContent()).includes("'  bonjour    le   monde  '"));
  checks.push('conversation restored');

  // 6. Refusing a plan runs nothing.
  await page.getByRole('button', { name: 'Nouvelle conversation' }).click();
  await message.fill('Génère un mot de passe de 20 caractères.');
  await page.getByRole('button', { name: 'Envoyer' }).click();
  await approve.waitFor({ timeout: 90000 });
  await page.getByRole('button', { name: 'Refuser' }).click();
  await page.getByText('Plan refusé.').waitFor();
  await page.locator('.mind-assistant').last().waitFor({ timeout: 90000 });
  assert.equal(await page.locator('.mind-result').count(), 0, 'a refused plan runs nothing');
  checks.push('refusal');
  assert.deepEqual(problems, []);
  console.log(`mind OK (real Gemma): ${checks.join(', ')}.`);
} catch (error) {
  // Show what the page and the model exchange looked like when a step timed out.
  const pages = browser.contexts().flatMap(c => c.pages());
  for (const page of pages) console.error('--- page log:\n' + (await page.locator('.mind-log').innerText().catch(() => '(no log)')));
  throw error;
} finally {
  await browser.close();
  server.close();
}
