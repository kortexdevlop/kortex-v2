// Production recipe for the hosting-only batch; does not create or use an account.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createRequire} from 'node:module';
const {chromium} = createRequire(import.meta.url)('playwright');
const text=readFileSync(new URL('../../brand/domains.toml',import.meta.url),'utf8');
const domains=Object.fromEntries([...text.matchAll(/^\s*(\w+)\s*=\s*"([^"]+)"/gm)].map(m=>[m[1],m[2]]));
const origin=`https://${domains.root}`;
const apiOrigin=`https://${domains.api}`;
const runOrigin=`https://${domains.run}`;
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_PATH||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
try {
  const page=await browser.newPage();
  const errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto(origin,{waitUntil:'networkidle'});
  await page.evaluate(async()=>{await navigator.serviceWorker.ready;});
  await page.goto(`${origin}/compte`,{waitUntil:'networkidle'});
  await page.locator('[data-view=auth]').waitFor({state:'visible'});
  assert.ok(await page.evaluate(()=>Boolean(navigator.serviceWorker.controller)));
  await page.context().setOffline(true);
  const offline=await page.evaluate(async apiOrigin=>{
    const read=async url=>{try{const r=await fetch(url,{credentials:'include'});return {status:r.status,text:await r.text()};}catch{return {networkError:true};}};
    return {api:await read(`${apiOrigin}/v1/me`),script:await read('/never-cached-script.js')};
  },apiOrigin);
  assert.equal(offline.api.networkError,true);
  assert.equal(offline.script.networkError,true);
  await page.reload({waitUntil:'networkidle'});
  await page.locator('[data-view=auth]').waitFor({state:'visible'});
  await page.context().setOffline(false);
  await page.goto(`${origin}/calculateur-hash?open=1`,{waitUntil:'networkidle'});
  const frame=page.frameLocator('[data-stage] iframe');
  await frame.getByLabel('Texte (si aucun fichier)',{exact:true}).waitFor();
  assert.equal(await frame.locator('html').evaluate(()=>document.referrer),`${origin}/`);
  await page.evaluate(()=>{document.documentElement.dataset.theme='dark';window.dispatchEvent(new Event('kortex:theme'));});
  const tool=page.frames().find(frame=>frame.url().startsWith(`${runOrigin}/`));
  await tool.waitForFunction(()=>document.documentElement.dataset.theme==='dark');
  assert.deepEqual(errors,[]);
  console.log('Hosting OK: offline API/script fail as network errors, account reloads offline, iframe receives only the host origin and live theme changes.');
} finally {await browser.close();}
