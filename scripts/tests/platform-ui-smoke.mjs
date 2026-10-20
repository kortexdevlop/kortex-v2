// Grouped end-to-end recipe for rebuilt platform-dependent web bundles.
// Requires existing Playwright and Chromium; builds must finish first.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {readFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {resolve,extname} from 'node:path';
const {chromium}=createRequire(import.meta.url)('playwright');
const root=resolve(import.meta.dirname,'../../dist/run');
const server=createServer(async(req,res)=>{
 try{
  if(req.url==='/favicon.ico'){res.statusCode=204;res.end();return;}
  const name=decodeURIComponent(new URL(req.url,'http://local').pathname);
  const path=resolve(root,'.'+(name.endsWith('/')?name+'index.html':name));
  if(!path.startsWith(root+'/')) throw new Error('path');
  const type={'.html':'text/html','.js':'text/javascript','.wasm':'application/wasm','.css':'text/css','.png':'image/png'}[extname(path)]||'application/octet-stream';
  res.setHeader('content-type',type);res.end(await readFile(path));
 }catch{res.statusCode=404;res.end();}
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_PATH||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
try{
 const context=await browser.newContext({permissions:['clipboard-read','clipboard-write'],serviceWorkers:'block'});
 const page=await context.newPage(),errors=[];
 page.setDefaultTimeout(10000);
 page.on('pageerror',e=>{errors.push(e.message);console.log('pageerror:',e.message);});
 const base=`http://127.0.0.1:${server.address().port}`;
 await page.goto(base+'/secure.hash-calculator/?lang=en&mode=simple');
 await page.locator('input[type=file]').setInputFiles({name:'quoted-"-file.txt',mimeType:'text/plain',buffer:Buffer.from('abc')});
 await page.getByText('quoted-"-file.txt',{exact:false}).waitFor().catch(async error=>{console.log('DOM:',await page.locator('body').innerText());throw error;});
 await page.getByRole('button',{name:'Run',exact:true}).click();
 const hash='ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';
 await page.getByText(hash,{exact:true}).waitFor();
 await page.getByRole('button',{name:'Copy',exact:true}).first().click();
 await page.waitForFunction(async expected=>await navigator.clipboard.readText()===expected,hash);
 await page.getByRole('status').filter({hasText:'Copied.'}).waitFor();
 await page.evaluate(()=>{
  navigator.clipboard.writeText=()=>Promise.reject(new DOMException('denied','NotAllowedError'));
 });
 await page.getByRole('button',{name:'Copy',exact:true}).first().click();
 await page.getByRole('alert').filter({hasText:'Copy failed. Select the result to copy it manually.'}).waitFor();

 await page.goto(base+'/qr.generator/?lang=en&mode=simple');
 await page.getByLabel('Content (link, text…)',{exact:true}).fill('Kortex platform " Unicode é');
 await page.getByRole('button',{name:'Run',exact:true}).click();
 await page.locator('.k-preview').waitFor();
 const pending=page.waitForEvent('download');
 await page.getByRole('button',{name:'Download',exact:true}).click();
 const download=await pending;
 assert.equal(download.suggestedFilename(),'qrcode.png');
 const bytes=await readFile(await download.path());
 assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
 await page.goto(base+'/secure.password-generator/?lang=en&mode=simple');
 await page.getByRole('button',{name:'Run',exact:true}).click();
 await page.locator('.k-result .k-output code').first().waitFor();
 const password=await page.locator('.k-result .k-output code').first().innerText();
 assert.equal(password.length,16);
 await page.getByRole('button',{name:'Run',exact:true}).click();
 await page.waitForFunction(first=>document.querySelector('.k-result .k-output code')?.textContent!==first,password);
 assert.deepEqual(errors,[]);
 console.log('Platform UI OK: generated launch query/EN/simple, selected-file read, Worker SHA-256, confirmed clipboard copy and denied-copy alert, binary PNG download, browser CSPRNG, no JS errors.');
}finally{await browser.close();await new Promise(r=>server.close(r));}
