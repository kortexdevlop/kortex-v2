// Minimal e2e smoke (MASTER_PROMPT §11): three key pages of the built site, served locally.
// Needs `pnpm site:build` first. No network, account or production host is involved.
// CHROME_PATH may point at an installed Chrome; otherwise Playwright's Chromium is used
// (`pnpm exec playwright install chromium`).
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { chromium } from 'playwright';

const dist = resolve(import.meta.dirname, '../../web/site/dist');
assert.ok(existsSync(resolve(dist, 'index.html')), 'web/site/dist missing: run `pnpm site:build` first');
const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.svg': 'image/svg+xml', '.wasm': 'application/wasm', '.webmanifest': 'application/manifest+json', '.xml': 'application/xml', '.txt': 'text/plain' };

/** Static files with the site's `trailingSlash: never` / directory layout. */
function resolveFile(pathname) {
  const base = resolve(dist, '.' + decodeURIComponent(pathname));
  if (base !== dist && !base.startsWith(dist + sep)) return null;
  for (const candidate of [base, resolve(base, 'index.html'), `${base}.html`]) {
    if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
  }
  return null;
}

const server = createServer((request, response) => {
  const file = resolveFile(new URL(request.url, 'http://localhost').pathname);
  if (!file) {
    response.writeHead(404).end('Not found');
    return;
  }
  response.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream' }).end(readFileSync(file));
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const origin = `http://127.0.0.1:${server.address().port}`;

const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || undefined });
const checks = [];
try {
  const context = await browser.newContext();
  // Third-party and other Kortex hosts (tool iframe origin, API) are out of scope: block them.
  await context.route(url => new URL(url).origin !== origin, route => route.abort());
  const page = await context.newPage();
  const problems = [];
  page.on('pageerror', error => problems.push(`pageerror: ${error.message}`));
  page.on('response', response => {
    if (new URL(response.url()).origin === origin && response.status() >= 400 && !response.url().endsWith('/favicon.ico')) {
      problems.push(`${response.status()} ${response.url()}`);
    }
  });

  // 1. Home: search palette finds a tool.
  const home = await page.goto(`${origin}/`, { waitUntil: 'networkidle' });
  assert.equal(home.status(), 200);
  assert.equal(await page.locator('html').getAttribute('lang'), 'fr');
  await page.locator('h1').first().waitFor();
  await page.locator('[data-seek-open]').first().click();
  await page.locator('[data-seek-input]').fill('sha256');
  await page.locator('[data-seek-list] a').first().waitFor();
  await page.keyboard.press('Escape');
  checks.push('home + search');

  // 2/3. The same tool in both languages: heading, hreflang pair, structured data, Stage trigger.
  for (const [path, lang, title] of [['/nettoyer-texte', 'fr', 'Nettoyeur de texte'], ['/en/text-cleaner', 'en', 'Text Cleaner']]) {
    const response = await page.goto(`${origin}${path}`, { waitUntil: 'networkidle' });
    assert.equal(response.status(), 200, path);
    assert.equal(await page.locator('html').getAttribute('lang'), lang, path);
    assert.equal((await page.locator('h1').first().textContent()).trim(), title, path);
    assert.equal(await page.locator('link[rel="alternate"][hreflang]').count(), 3, `${path} hreflang`);
    const types = (await page.locator('script[type="application/ld+json"]').allTextContents()).join(' ');
    assert.match(types, /SoftwareApplication/, `${path} JSON-LD`);
    assert.match(types, /FAQPage/, `${path} JSON-LD`);
    assert.ok(await page.locator('[data-stage-open]').count() > 0, `${path} open button`);
    checks.push(`tool page ${lang}`);
  }
  assert.deepEqual(problems, []);
  console.log(`e2e smoke OK: ${checks.join(', ')}.`);
} finally {
  await browser.close();
  server.close();
}
