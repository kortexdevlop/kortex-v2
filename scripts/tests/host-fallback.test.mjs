import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../../crates/kortex-platform/src/host-bus.js',import.meta.url),'utf8');
test('host bus works without ancestorOrigins and rejects forged origins/windows',()=>{
  const sent=[], modes=[],listeners={}, dataset={};
  const parent={postMessage:(msg,origin)=>sent.push([msg,origin])};
  const window={parent,addEventListener:(type,fn)=>listeners[type]=fn};
  vm.runInNewContext(source,{window,location:{},document:{referrer:'https://kortex.site/',documentElement:{dataset}},URL,dioxus:{send:mode=>modes.push(mode)}});
  assert.equal(sent[0][0].type,'kortex:ready');
  assert.equal(sent[0][1],'https://kortex.site');
  const data={type:'kortex:init',theme:'dark',mode:'pro'};
  listeners.message({source:parent,origin:'https://evil.example',data});
  listeners.message({source:{},origin:'https://kortex.site',data});
  assert.deepEqual(dataset,{});
  listeners.message({source:parent,origin:'https://kortex.site',data});
  assert.equal(dataset.theme,'dark');
  assert.deepEqual(modes,['pro']);
});

test('platform download preserves binary/name and releases the object URL',()=>{
  const template=readFileSync(new URL('../../crates/kortex-platform/src/download.js',import.meta.url),'utf8');
  const name='quote"\\\n\u2028;throw new Error("injected")';
  const events=[];
  let blob;
  const anchor={click(){events.push('click');},remove(){events.push('remove');}};
  const context={
    atob:value=>Buffer.from(value,'base64').toString('binary'),Uint8Array,
    Blob:class {constructor(parts,options){blob={bytes:[...parts[0]],mime:options.type};}},
    URL:{createObjectURL(){return 'blob:test';},revokeObjectURL(url){events.push(url);}},
    document:{createElement(){return anchor;},body:{appendChild(){events.push('append');}}},
    setTimeout:callback=>callback(),
  };
  const script=template.replace('__KORTEX_ARGS__',JSON.stringify(['AP8=','application/octet-stream',name]));
  vm.runInNewContext(script,context);
  assert.deepEqual(blob,{bytes:[0,255],mime:'application/octet-stream'});
  assert.equal(anchor.download,name);
  assert.equal(anchor.href,'blob:test');
  assert.deepEqual(events,['append','click','remove','blob:test']);
  events.length=0;
  anchor.click=()=>{throw new Error('cancelled');};
  assert.throws(()=>vm.runInNewContext(script,context),/cancelled/);
  assert.deepEqual(events,['append','remove','blob:test']);
});

test('application host validates origin and exact iframe, and removes its listener',async()=>{
  const template=readFileSync(new URL('../../crates/kortex-platform/src/app-host.js',import.meta.url),'utf8');
  const sent=[],ready=[],listeners={};
  let stop,removed=false;
  const child={postMessage:(data,origin)=>sent.push([JSON.parse(JSON.stringify(data)),origin])};
  const iframe={dataset:{kortexWindow:'7',kortexMode:'pro',kortexLang:'en'},contentWindow:child};
  const context={
    window:{addEventListener:(type,callback)=>listeners[type]=callback,removeEventListener:(type,callback)=>{removed=listeners[type]===callback;}},
    document:{querySelectorAll:()=>[iframe],documentElement:{dataset:{theme:'dark'}}},
    dioxus:{send:data=>ready.push(JSON.parse(JSON.stringify(data))),recv:()=>new Promise(resolve=>stop=resolve)},
  };
  const running=vm.runInNewContext(`(async()=>{${template.replace('__KORTEX_ARGS__',JSON.stringify([7,'https://run.kortex.site']))}})()`,context);
  const data={type:'kortex:ready'};
  listeners.message({origin:'https://evil.example',source:child,data});
  listeners.message({origin:'https://run.kortex.site',source:{},data});
  listeners.message({origin:'https://run.kortex.site',source:child,data:{type:'wrong'}});
  assert.deepEqual(sent,[]);assert.deepEqual(ready,[]);
  listeners.message({origin:'https://run.kortex.site',source:child,data});
  assert.deepEqual(ready,[{version:1,frame:7}]);
  assert.deepEqual(sent,[[{type:'kortex:init',mode:'pro',lang:'en',theme:'dark',size:'normal'},'https://run.kortex.site']]);
  stop('stop');await running;assert.equal(removed,true);
});
