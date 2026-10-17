// One grouped production recipe after a deployment; no personal profile or account is used.
import {publicFeatures} from '../public-features.mjs';
import {accountFlow} from './account-flow.mjs';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
const {chromium} = createRequire(import.meta.url)('playwright');
const root = resolve(import.meta.dirname, '../..');
const domains = Object.fromEntries([...readFileSync(resolve(root, 'brand/domains.toml'), 'utf8').matchAll(/^\s*(\w+)\s*=\s*"([^"]+)"/gm)].map(m => [m[1],m[2]]));
const origin = process.env.KORTEX_SITE_ORIGIN || `https://${domains.root}`;
const browser = await chromium.launch({headless:true,args:process.env.KORTEX_QA_HOST_RULES ? [`--host-resolver-rules=${process.env.KORTEX_QA_HOST_RULES}`] : [], executablePath:process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
try {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const home = await page.goto(origin, {waitUntil:'networkidle', timeout:60000});
  assert.equal(home.status(),200);
  assert.ok(home.headers()['content-security-policy']);
  await page.locator('[data-seek-open]').first().click();
  await page.locator('[data-seek-input]').fill('sha256');
  await page.locator('[data-seek-list] a').first().waitFor();
  await page.keyboard.press('Escape');
  if (publicFeatures(root).account) await accountFlow(page, origin, `https://${domains.api}`);
  else {
    await page.goto(`${origin}/compte`, {waitUntil:'networkidle'});
    await page.getByRole('heading', {name:'Le compte arrive bientôt'}).waitFor();
    assert.equal(await page.locator('form').count(),0);
  }
  await page.goto(`${origin}/en/download`, {waitUntil:'networkidle'});
  await page.getByRole('heading', {name:'The desktop version is being prepared'}).waitFor();
  assert.equal((await page.request.get(`${origin}/install.sh`)).status(),404);
  await page.goto(`${origin}/calculateur-hash?open=1`, {waitUntil:'networkidle'});
  const tool = page.frameLocator('[data-stage] iframe');
  await tool.getByLabel('Texte (si aucun fichier)', {exact:true}).fill('abc');
  await tool.getByRole('button', {name:'Lancer',exact:true}).click();
  await tool.getByText('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad', {exact:true}).waitFor();
  assert.deepEqual(errors,[]);
  console.log('Production OK: HTTPS, security headers, search, account availability and unavailable desktop downloads, no test installer, iframe and Worker SHA-256.');
} finally {await browser.close();}
