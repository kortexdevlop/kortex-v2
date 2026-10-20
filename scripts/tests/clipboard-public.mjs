// One public recipe for the rebuilt shared clipboard UI in its real cross-origin Stage.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
const {chromium}=createRequire(import.meta.url)('playwright');
const root=resolve(import.meta.dirname,'../..');
const domains=Object.fromEntries([...readFileSync(resolve(root,'brand/domains.toml'),'utf8').matchAll(/^\s*(\w+)\s*=\s*"([^"]+)"/gm)].map(m=>[m[1],m[2]]));
const tool=JSON.parse(readFileSync(resolve(root,'registry/contracts.json'))).tools.find(entry=>entry.tool.id==='secure.hash-calculator').tool;
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_PATH||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',args:process.env.KORTEX_QA_HOST_RULES?[`--host-resolver-rules=${process.env.KORTEX_QA_HOST_RULES}`]:[]});
try{
 const context=await browser.newContext({permissions:['clipboard-read','clipboard-write']});
 const page=await context.newPage(),errors=[];
 page.on('pageerror',e=>errors.push(e.message));
 const hash='ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';
 for(const lang of ['fr','en']){
  const url=`https://${domains.root}/${lang==='en'?'en/':''}${tool.slug[lang]}?open=1`;
  const response=await page.goto(url,{waitUntil:'networkidle',timeout:60000});
  assert.equal(response.status(),200);
  const frame=page.frameLocator('[data-stage] iframe');
  await frame.getByLabel(lang==='fr'?'Texte (si aucun fichier)':'Text (if no file)',{exact:true}).fill('abc');
  await frame.getByRole('button',{name:lang==='fr'?'Lancer':'Run',exact:true}).click();
  await frame.getByText(hash,{exact:true}).waitFor();
  await frame.getByRole('button',{name:lang==='fr'?'Copier':'Copy',exact:true}).first().click();
  await frame.getByRole('status').filter({hasText:lang==='fr'?'Copié.':'Copied.'}).waitFor();
  assert.equal(await page.evaluate(()=>navigator.clipboard.readText()),hash);
  const hostFrame=page.frames().find(candidate=>candidate.url().startsWith(`https://${domains.run}/secure.hash-calculator/`));
  assert.ok(hostFrame);
  await hostFrame.evaluate(()=>{navigator.clipboard.writeText=()=>Promise.reject(new DOMException('denied','NotAllowedError'));});
  await frame.getByRole('button',{name:lang==='fr'?'Copier':'Copy',exact:true}).first().click();
  await frame.getByRole('alert').filter({hasText:lang==='fr'?'Copie impossible.':'Copy failed.'}).waitFor();
  assert.equal(await frame.getByRole('status').filter({hasText:lang==='fr'?'Copié.':'Copied.'}).count(),0);
  assert.equal(await page.evaluate(()=>navigator.clipboard.readText()),hash);
 }
 assert.deepEqual(errors,[]);
 console.log('Public clipboard UI OK: FR/EN, actual cross-origin Stage and Rust Worker, confirmed copy, denied copy with visible alert and no false success, no account created.');
}finally{await browser.close();}
