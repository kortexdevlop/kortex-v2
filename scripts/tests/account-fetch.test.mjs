import {test} from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';
const template=readFileSync(new URL('../../crates/kortex-platform/src/account-fetch.js',import.meta.url),'utf8');
async function run(fetch,limit=100){
 const script=template.replace('__KORTEX_ARGS__',JSON.stringify(['https://api.kortex.site/v1/me','GET',null,limit,'user-id']));
 return vm.runInNewContext(`(async()=>{${script}})()`,{fetch,AbortController,setTimeout,clearTimeout,TextDecoder});
}
test('account delivery uses protected cookies, expected identity, no redirects/cache and bounded UTF-8',async()=>{
 const reply=await run(async(url,options)=>{assert.equal(url,'https://api.kortex.site/v1/me');assert.equal(options.credentials,'include');assert.equal(options.redirect,'error');assert.equal(options.cache,'no-store');assert.equal(options.headers['X-Kortex-User'],'user-id');return new Response('{"text":"é"}');});
 assert.equal(reply.status,200);assert.deepEqual(JSON.parse(reply.body),{text:'é'});
});
test('account delivery rejects oversized declared/streamed responses and network failures',async()=>{
 let signal;assert.equal((await run(async(_url,options)=>{signal=options.signal;return new Response('x',{headers:{'content-length':'101'}});})).status,0);assert.equal(signal.aborted,true);
 assert.equal((await run(async()=>new Response('é'.repeat(51)))).status,0);
 assert.equal((await run(async()=>{throw new Error('offline');})).status,0);
});
