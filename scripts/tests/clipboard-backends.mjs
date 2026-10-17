// Grouped test of the actual Rust clipboard backend with real browser operations.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {spawnSync} from 'node:child_process';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {createServer} from 'node:http';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const root=resolve(import.meta.dirname,'../..');
const output=await mkdtemp(join(tmpdir(),'kortex-clipboard-web-'));
let server,browser;
function run(command,args){
 const result=spawnSync(command,args,{cwd:root,stdio:'inherit',env:{...process.env,CARGO_NET_OFFLINE:'true'}});
 if(result.error)throw result.error;
 assert.equal(result.status,0,command);
}
try{
 run('cargo',['build','--locked','--manifest-path','scripts/tests/fixtures/storage-probe/Cargo.toml','--target','wasm32-unknown-unknown','--target-dir',join(root,'target/storage-probe')]);
 run('wasm-bindgen',[join(root,'target/storage-probe/wasm32-unknown-unknown/debug/kortex_storage_probe.wasm'),'--target','web','--out-dir',output]);
 server=createServer(async(req,res)=>{
  try{
   if(req.url==='/probe.js'||req.url==='/probe.wasm'){
    const wasm=req.url.endsWith('.wasm');
    res.setHeader('content-type',wasm?'application/wasm':'text/javascript');
    res.end(await readFile(join(output,wasm?'kortex_storage_probe_bg.wasm':'kortex_storage_probe.js')));
   }else{res.setHeader('content-type','text/html');res.end('<!doctype html><script type="module">import init,*as api from "/probe.js";await init({module_or_path:"/probe.wasm"});window.probe=api;</script>');}
  }catch{res.statusCode=500;res.end();}
 });
 await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
 const {chromium}=createRequire(import.meta.url)('playwright');
 browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_PATH||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
 const context=await browser.newContext({permissions:['clipboard-read','clipboard-write']});
 const page=await context.newPage(),url=`http://127.0.0.1:${server.address().port}`,errors=[];
 page.on('pageerror',e=>errors.push(e.message));
 await page.goto(url);await page.waitForFunction(()=>window.probe);
 const text = 'Kortex \" quotes;\nUnicode é 🌙';
 await page.evaluate(async text=>{await probe.clipboard_write(text);},text);
 assert.equal(await page.evaluate(()=>probe.clipboard_read()),text);
 await page.evaluate(()=>probe.clipboard_write(''));
 assert.equal(await page.evaluate(()=>probe.clipboard_read()),'');
 const failures=await page.evaluate(async()=>{
  const denied=async operation=>{try{await operation();return 'unexpected_success';}catch(error){return String(error);}};
  const original=navigator.clipboard.writeText;
  navigator.clipboard.writeText=()=>Promise.reject(new DOMException('denied','NotAllowedError'));
  const rejection=await denied(()=>probe.clipboard_write('do not copy'));
  navigator.clipboard.writeText=()=>{throw new DOMException('denied','SecurityError');};
  const thrown=await denied(()=>probe.clipboard_write('do not copy'));
  navigator.clipboard.writeText=original;
  const untouched=await probe.clipboard_read();
  Object.defineProperty(navigator,'clipboard',{value:undefined,configurable:true});
  const unavailable=await denied(()=>probe.clipboard_read());
  return {rejection,thrown,untouched,unavailable};
 });
 assert.deepEqual(failures,{rejection:'clipboard_denied',thrown:'clipboard_denied',untouched:'',unavailable:'clipboard_unavailable'});
 assert.deepEqual(errors,[]);
 console.log('Rust clipboard OK: real write/read, Unicode, empty text, rejected promise, synchronous denial, unavailable API, no false success or uncaught errors.');
}finally{
 await browser?.close();
 if(server?.listening)await new Promise(r=>server.close(r));
 await rm(output,{recursive:true,force:true});
}
