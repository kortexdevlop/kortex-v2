// Installability of the compiled web apps (dist/apps/*): a valid manifest with usable icons, a
// service worker that takes control, and a shell that still opens with the network off.
// Needs `cargo xtask build-app apps.<name> --web` for the apps to check (all found are checked).
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { chromium } from 'playwright';

const apps = resolve(import.meta.dirname, '../../dist/apps');
const ids = existsSync(apps) ? readdirSync(apps).filter(id => existsSync(resolve(apps, id, 'manifest.webmanifest'))) : [];
assert.ok(ids.length > 0, 'no app built with a manifest: run `cargo xtask build-app apps.<name> --web`');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || undefined });
const checked = [];
try {
  for (const id of ids) {
    const dist = resolve(apps, id);
    const server = createServer((request, response) => {
      let path = resolve(dist, '.' + decodeURIComponent(new URL(request.url, 'http://x').pathname));
      if (path !== dist && !path.startsWith(dist + sep)) return response.writeHead(403).end();
      if (existsSync(path) && statSync(path).isDirectory()) path = resolve(path, 'index.html');
      if (!existsSync(path)) return response.writeHead(404).end();
      response.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' }).end(readFileSync(path));
    });
    await new Promise(done => server.listen(0, '127.0.0.1', done));
    const origin = `http://127.0.0.1:${server.address().port}`;
    const context = await browser.newContext();
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(origin + '/', { waitUntil: 'networkidle' });

    // Manifest: linked, valid and installable (name, standalone, start URL in scope, 192 + 512 icons).
    const href = await page.locator('link[rel="manifest"]').getAttribute('href');
    const manifest = await (await page.request.get(new URL(href, origin).href)).json();
    assert.ok(manifest.name.startsWith('Kortex '), `${id} name`);
    assert.equal(manifest.display, 'standalone');
    assert.equal(manifest.start_url, '/');
    for (const size of ['192x192', '512x512']) {
      const icon = manifest.icons.find(i => i.sizes === size);
      assert.ok(icon, `${id} icon ${size}`);
      const reply = await page.request.get(new URL(icon.src, origin).href);
      assert.equal(reply.status(), 200);
      assert.equal(reply.headers()['content-type'], 'image/png');
      const png = await reply.body();
      assert.equal(png.readUInt32BE(16), Number(size.split('x')[0]), `${id} icon ${size} has its size`);
    }
    assert.equal((await page.request.get(`${origin}/icons/apple-touch-icon.png`)).status(), 200);

    // Service worker: registers, activates and controls the page.
    await page.evaluate(() => navigator.serviceWorker.ready.then(() => true));
    await page.reload({ waitUntil: 'networkidle' });
    assert.ok(await page.evaluate(() => navigator.serviceWorker.controller !== null), `${id}: the worker controls the page`);
    const cached = await page.evaluate(async () => (await caches.keys()).filter(k => k.startsWith('kortex-app:')));
    assert.ok(cached.length >= 1, `${id}: an app cache exists`);

    // Offline: the shell opens (the app itself loads from the cache) in both languages.
    await context.setOffline(true);
    for (const path of ['/', '/en/']) {
      const response = await page.goto(origin + path, { waitUntil: 'load' });
      assert.ok(response && response.ok(), `${id} ${path} offline`);
      await page.locator('#main h1').first().waitFor({ timeout: 20000 });
    }
    assert.deepEqual(errors, [], id);
    await context.setOffline(false);
    await context.close();
    server.close();
    checked.push(id);
  }
  console.log(`pwa OK: ${checked.join(', ')} (manifest, icons, worker control, offline shell).`);
} finally {
  await browser.close();
}
