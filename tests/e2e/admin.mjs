// The owner's dashboard (web/site/dist/admin) against a simulated overview: sections, the
// denied state for a normal visitor, the rollback command copy. Needs `pnpm site:build`.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { chromium } from 'playwright';

const dist = resolve(import.meta.dirname, '../../web/site/dist');
assert.ok(existsSync(resolve(dist, 'admin/index.html')), 'run `pnpm site:build` first');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.svg': 'image/svg+xml' };
const server = createServer((request, response) => {
  let path = resolve(dist, '.' + decodeURIComponent(new URL(request.url, 'http://x').pathname));
  if (path !== dist && !path.startsWith(dist + sep)) return response.writeHead(403).end();
  if (existsSync(path) && statSync(path).isDirectory()) path = resolve(path, 'index.html');
  if (!existsSync(path)) return response.writeHead(404).end();
  response.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' }).end(readFileSync(path));
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const origin = `http://127.0.0.1:${server.address().port}`;
const overview = {
  generated_at: Date.now(),
  api_version: { id: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee', deployed_at: '2026-10-20T10:00:00Z' },
  hosts: [{ host: 'kortex.site', ok: true, status: 200, ms: 85 }, { host: 'agent.kortex.site', ok: false, status: 0, ms: 4000 }],
  ai: { day: '2026-10-20', global: { used: 7200, cap: 9000, percent: 80, factor: 0.5 }, today: { requests: 41, users: 6, top: [{ user: 'ab12cd34', neurons: 900, requests: 12 }] }, apps: [{ app: 'flow', requests: 30, neurons: 2100 }, { app: 'mind', requests: 11, neurons: 800 }], days: [{ day: '2026-10-19', requests: 20, neurons: 1500 }, { day: '2026-10-20', requests: 41, neurons: 3100 }], low_credit_users: 2 },
  errors: [{ day: '2026-10-20', app: 'flow', kind: 'panic', n: 4 }],
  database: { users: 12, sessions: 9, sync_items: 31, ai_buckets: 6, rate_limits: 4 },
  releases: [{ file: 'x.json', date: '2026-10-20', run: '39330128948', commit: 'f665a34311', versions: { 'kortex-api': '651da94e-ea3f-4afe-b6b9-ce2634a2d5bc', 'kortex-flow': 'f5455379-b480-4ca9-b623-4d8c7c560d8d' } }],
};
const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || undefined });
const checks = [];
try {
  const context = await browser.newContext({ locale: 'fr-FR', permissions: ['clipboard-read', 'clipboard-write'] });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  let answer = { status: 200, body: overview };
  await page.route('https://api.kortex.site/v1/admin/overview', async route => {
    const cors = { 'access-control-allow-origin': origin, 'access-control-allow-credentials': 'true', 'access-control-allow-headers': 'content-type, x-kortex-request', 'access-control-allow-methods': 'GET' };
    if (route.request().method() === 'OPTIONS') return route.fulfill({ status: 204, headers: cors });
    await route.fulfill({ status: answer.status, headers: { ...cors, 'content-type': 'application/json' }, body: JSON.stringify(answer.body) });
  });
  await page.goto(`${origin}/admin/`, { waitUntil: 'networkidle' });
  await page.getByRole('heading', { name: 'Sites' }).waitFor();
  assert.ok((await page.locator('.pill[data-ok="true"]').first().textContent()).includes('en ligne'));
  assert.ok((await page.locator('.pill[data-ok="false"]').first().textContent()).includes('hors ligne'));
  await page.getByText('7200 / 9000 neurons (80 %)').waitFor();
  await page.getByText('Recharge ralentie ×0.5').waitFor();
  assert.equal(await page.locator('.bar').getAttribute('data-tone'), 'warn');
  await page.getByRole('cell', { name: 'ab12cd34' }).waitFor();
  await page.getByRole('cell', { name: 'flow' }).first().waitFor();
  checks.push('health, AI budget and consumers');
  await page.getByRole('heading', { name: /Erreurs des applications/ }).waitFor();
  await page.getByRole('cell', { name: 'panic: 4' }).waitFor();
  await page.getByText('12', { exact: true }).first().waitFor();
  checks.push('errors and database');
  await page.locator('button', { hasText: 'Copier la commande' }).first().click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  assert.equal(copied, 'wrangler rollback 651da94e-ea3f-4afe-b6b9-ce2634a2d5bc --config services/api/wrangler.jsonc --env production');
  checks.push('rollback command copied');
  answer = { status: 404, body: { error: 'not_found' } };
  await page.getByRole('button', { name: 'Actualiser' }).click();
  await page.getByText('Accès réservé').waitFor();
  assert.ok(await page.locator('[data-board]').isHidden(), 'nothing is shown to a visitor');
  checks.push('a normal visitor sees nothing');
  const robots = await page.locator('meta[name="robots"]').getAttribute('content');
  assert.equal(robots, 'noindex');
  assert.deepEqual(errors, []);
  console.log(`admin OK: ${checks.join(', ')}.`);
} finally {
  await browser.close();
  server.close();
}
