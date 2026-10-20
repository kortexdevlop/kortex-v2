// Display preferences cookie: nothing is stored before a choice; a choice persists across pages and is shared
// through the `k-prefs` cookie (theme, mode, text size); the footer link removes it. Usage: node tests/e2e/prefs.mjs [base-url]
import { chromium, devices } from 'playwright';
import assert from 'node:assert/strict';
const base = process.argv[2] ?? 'http://localhost:4321';
const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || undefined });
for (const [label, opts] of [['desktop', { viewport: { width: 1280, height: 800 } }], ['mobile', { ...devices['Pixel 7'], viewport: { width: 390, height: 760 } }]]) {
  const ctx = await browser.newContext({ ...opts, colorScheme: 'light' });
  const page = await ctx.newPage();
  const errors = []; page.on('pageerror', (e) => errors.push(e.message));
  const cookie = async () => (await ctx.cookies()).find((c) => c.name === 'k-prefs');
  await page.goto(base + '/', { waitUntil: 'networkidle' });
  assert.equal(await cookie(), undefined, `${label}: no cookie before any choice`);
  const open = async () => { if (await page.locator('[data-settings-toggle]').isVisible()) { if ((await page.locator('[data-settings-toggle]').getAttribute('aria-expanded')) !== 'true') await page.locator('[data-settings-toggle]').click(); await page.waitForTimeout(350); } };
  await open();
  await page.locator('[data-theme-toggle]').click(); // auto -> light
  await page.locator('[data-theme-toggle]').click(); // light -> dark
  assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'dark');
  await page.locator('[data-mode="pro"]').click();
  await page.locator('[data-size-toggle]').click();
  const c = await cookie();
  assert.ok(c, `${label}: cookie set after a choice`);
  assert.equal(decodeURIComponent(c.value), 'm=pro&s=large&t=dark', `${label}: payload`);
  assert.equal(c.sameSite, 'Lax'); assert.ok(c.expires > Date.now() / 1000 + 300 * 86400, 'one year');
  assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).fontSize), '18px');
  // persists across reload and other pages
  await page.goto(base + '/calcul-pourcentage', { waitUntil: 'networkidle' });
  assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'dark');
  assert.equal(await page.evaluate(() => document.documentElement.dataset.size), 'large');
  await open();
  assert.equal(await page.locator('[data-mode="pro"]').getAttribute('aria-checked'), 'true');
  // simulate another host: same cookie, empty localStorage
  const ctx2 = await browser.newContext({ ...opts, colorScheme: 'light' });
  await ctx2.addCookies([{ ...c, url: base }].map(({ name, value }) => ({ name, value, url: base })));
  const p2 = await ctx2.newPage(); await p2.goto(base + '/', { waitUntil: 'networkidle' });
  assert.equal(await p2.evaluate(() => document.documentElement.dataset.theme), 'dark', `${label}: cookie alone restores the theme`);
  await ctx2.close();
  // reset
  await page.locator('[data-prefs-reset]').click();
  assert.equal(await cookie(), undefined, `${label}: reset removes the cookie`);
  assert.equal(await page.evaluate(() => document.documentElement.dataset.theme ?? ''), '');
  assert.equal(await page.evaluate(() => document.documentElement.dataset.size ?? ''), '');
  assert.deepEqual(errors, []);
  console.log(label, 'OK');
  await ctx.close();
}
await browser.close();
