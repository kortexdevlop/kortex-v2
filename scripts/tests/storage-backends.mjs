// End-to-end browser test of the actual Rust WebStorage backend, not a JS substitute.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {spawnSync} from 'node:child_process';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {createServer} from 'node:http';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const root=resolve(import.meta.dirname,'../..');
const output=await mkdtemp(join(tmpdir(),'kortex-storage-web-'));
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
 const page=await browser.newPage(),url=`http://127.0.0.1:${server.address().port}`,errors=[];
 page.on('pageerror',e=>errors.push(e.message));
 await page.goto(url);await page.waitForFunction(()=>window.probe);
 assert.equal(await page.evaluate(()=>probe.get('desk','theme')),undefined);
 await page.evaluate(()=>{probe.set('desk','theme','dark 🌙');probe.set('desk:other','../outside','isolated');});
 await page.reload();await page.waitForFunction(()=>window.probe);
 assert.equal(await page.evaluate(()=>probe.get('desk','theme')),'dark 🌙');
 assert.equal(await page.evaluate(()=>probe.get('desk:other','theme')),undefined);
 assert.equal(await page.evaluate(()=>probe.get('desk','../outside')),undefined);
 assert.equal(await page.evaluate(()=>probe.get('desk:other','../outside')),'isolated');
 const results=await page.evaluate(()=>{
  const rejects=fn=>{try{fn();return false;}catch{return true;}};
  probe.set('desk','empty','');probe.set('desk','limit','x'.repeat(65536));
  const bounded=rejects(()=>probe.set('desk','limit','x'.repeat(65537)))&&probe.get('desk','limit').length===65536;
  const invalid=rejects(()=>probe.set('','theme','x'))&&rejects(()=>probe.get('desk','é'.repeat(65)));
  const empty=probe.get('desk','empty')==='';
  probe.remove('desk','theme');probe.remove('desk','theme');
  const removed=probe.get('desk','theme')===undefined;
  const original=Storage.prototype.setItem;
  Storage.prototype.setItem=function(){throw new DOMException('quota','QuotaExceededError');};
  const quota=rejects(()=>probe.set('desk','limit','replacement'));
  Storage.prototype.setItem=original;
  Object.defineProperty(window,'localStorage',{get(){throw new DOMException('denied','SecurityError');},configurable:true});
  const denied=rejects(()=>probe.get('desk','limit'));
  return {bounded,invalid,empty,removed,quota,denied};
 });
 assert.deepEqual(results,{bounded:true,invalid:true,empty:true,removed:true,quota:true,denied:true});
 assert.deepEqual(errors,[]);
 console.log('Rust WebStorage OK: reload persistence, namespaces, Unicode/path-like keys, empty values, bounds, removal, quota and denied access.');
}finally{
 await browser?.close();
 if(server?.listening)await new Promise(r=>server.close(r));
 await rm(output,{recursive:true,force:true});
}
