import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source = readFileSync(new URL('../../web/site/public/sw.js',import.meta.url),'utf8');
function setup() {
  const handlers={}, entries=new Map([['/',new Response('home')]]), writes=[];
  const context = {self:{location:{origin:'https://kortex.site'},addEventListener:(type,fn)=>handlers[type]=fn},caches:{open:async()=>({match:async req=>entries.get(typeof req==='string'?req:new URL(req.url).pathname)?.clone(),put:async(req,res)=>writes.push(req.url)})},fetch:async()=>{throw Error('offline');},Response,URL};
  vm.runInNewContext(source,context);
  async function request(url, mode='cors') {
    let response;
    const waits=[];
    handlers.fetch({request:{url,method:'GET',mode},respondWith:r=>response=r,waitUntil:r=>waits.push(r)});
    const value=response ? await response : undefined;
    await Promise.all(waits);
    return value;
  }
  return {context,entries,writes,request};
}
test('cross-origin API is untouched; offline HTML fallback is navigation-only',async()=>{
  const h=setup();
  assert.equal(await h.request('https://api.kortex.site/v1/me'),undefined);
  assert.equal((await h.request('https://kortex.site/uncached.js')).type,'error');
  assert.equal(await (await h.request('https://kortex.site/missing','navigate')).text(),'home');
  h.entries.set('/known.js',new Response('cached script'));
  assert.equal(await (await h.request('https://kortex.site/known.js')).text(),'cached script');
});
test('private/no-store responses are never cached',async()=>{
  const h=setup();
  h.context.fetch=async()=>new Response('private',{headers:{'Cache-Control':'private, no-store'}});
  await h.request('https://kortex.site/private');
  assert.deepEqual(h.writes,[]);
});
