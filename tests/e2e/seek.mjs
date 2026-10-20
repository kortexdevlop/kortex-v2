// Kortex Seek recipe on the compiled web app (dist/apps/apps.seek), served locally.
// Needs `cargo xtask build-app apps.seek --web`. CHROME_PATH may point at an installed Chrome.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { chromium } from 'playwright';

const dist = resolve(import.meta.dirname, '../../dist/apps/apps.seek');
assert.ok(existsSync(resolve(dist, 'index.html')), 'dist/apps/apps.seek missing: run `cargo xtask build-app apps.seek --web`');
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
try {
  for (const [locale, entry, cases] of [
    ['fr-FR', '/', [
      ['png en jpg', 'Convertir PNG en JPG', /\/convertir-png-en-jpg\?open=1$/],
      ['jpg en png', 'Convertir JPG en PNG', /\/convertir-jpg-en-png\?open=1$/],
      ['mdp', 'Générateur de mots de passe', /\?open=1$/],
      ['fusioner pdf', 'Fusionner des PDF', /\?open=1$/],
      ['réduire le poids d’une photo pour l’envoyer par mail', "Convertisseur d'images", /\?open=1$/],
    ]],
    ['en-US', '/en/', [
      ['merge several pdf files', 'Merge PDF', /\/en\/.*\?open=1$/],
      ['generate a strong password', 'Password Generator', /\/en\/.*\?open=1$/],
    ]],
  ]) {
    const context = await browser.newContext({ locale });
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(origin + entry, { waitUntil: 'networkidle' });
    const input = page.locator('input[type="search"]');
    await input.waitFor();
    // Empty state: examples taken from the contracts' own intents.
    assert.ok(await page.locator('.seek-hints button').count() >= 3, `${locale} suggestions`);
    for (const [query, title, href] of cases) {
      await input.fill(query);
      const first = page.locator('.app-list li').first();
      await first.waitFor();
      assert.equal((await first.locator('strong').textContent()).trim(), title, `${locale} “${query}”`);
      assert.match(await first.locator('a').getAttribute('href'), href, `${locale} “${query}” link`);
      checks.push(`${locale}:${query}`);
    }
    await input.fill('zzzqqq');
    await page.locator('[role="status"]').first().waitFor();
    assert.equal(await page.locator('.app-list li').count(), 0);
    await input.fill('');
    await page.locator('.seek-hints button').first().click();
    await page.locator('.app-list li').first().waitFor();
    assert.deepEqual(errors, []);
    await context.close();
  }
  console.log(`seek OK: ${checks.length} searches (FR + EN), empty state, suggestions.`);
} finally {
  await browser.close();
  server.close();
}
