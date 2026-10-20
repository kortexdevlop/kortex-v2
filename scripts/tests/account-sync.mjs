// One end-to-end recipe for the compiled Rust account clients and metadata-only backups.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {readFileSync,existsSync,statSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve,extname,sep} from 'node:path';
import {randomUUID} from 'node:crypto';
import {handle} from '../../services/api/src/index.ts';
import {makeEnv} from '../../services/api/tests/helpers.ts';
import {deriveAuthKey} from '../../services/shared/src/auth.ts';
const {chromium}=createRequire(import.meta.url)('playwright');
const root=resolve(import.meta.dirname,'../..'),published=process.argv.includes('--public');
const domains=Object.fromEntries([...readFileSync(resolve(root,'brand/domains.toml'),'utf8').matchAll(/^\s*(\w+)\s*=\s*"([^"]+)"/gm)].map(m=>[m[1],m[2]]));
const api=`https://${domains.api}`,origin=`https://${domains.root}`;
const {env}=makeEnv({ALLOWED_ORIGINS:Object.values(domains).map(h=>`https://${h}`).join(',')});
const users=[],errors=[],results=[],writes=[];
async function invoke(method,path,body,cookie,user){
 const headers={'Origin':origin,'X-Kortex-Request':'1',...(body===undefined?{}:{'Content-Type':'application/json'}),...(cookie?{Cookie:cookie}:{}),...(user?{'X-Kortex-User':user}:{})};
 const req=new Request(api+path,{method,headers,...(body===undefined?{}:{body:JSON.stringify(body)})});
 const response=published?await fetch(req):await handle(req,env);
 return {status:response.status,data:await response.json(),cookie:response.headers.get('set-cookie')?.split(';')[0]};
}
async function register(){
 const email=`qa-sync-${randomUUID()}@example.invalid`,password=`QA-${randomUUID()}`;
 const proof={email,auth_key:await deriveAuthKey(email,password,published?600_000:1000),kdf:1};
 const response=await invoke('POST','/v1/auth/register',proof);
 assert.equal(response.status,201,`QA registration: ${response.data.error??response.status}`);
 const fixture={proof,cookie:response.cookie,id:response.data.id,email};users.push(fixture);return fixture;
}
async function addCookie(context,fixture){
 const [name,...value]=fixture.cookie.split('=');
 await context.addCookies([{url:api,name,value:value.join('='),httpOnly:true,secure:true,sameSite:'Lax'}]);
}
const browser=await chromium.launch({headless:true,executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',args:process.env.KORTEX_QA_HOST_RULES?[`--host-resolver-rules=${process.env.KORTEX_QA_HOST_RULES}`]:[]});
async function context(fixture){
 const ctx=await browser.newContext({viewport:{width:1600,height:1100}});if(fixture)await addCookie(ctx,fixture);
 if(!published)await ctx.route('https://*.kortex.site/**',async route=>{
  const request=route.request(),url=new URL(request.url());
  if(url.hostname===domains.api){
   const headers=await request.allHeaders(),body=request.postDataBuffer();
   const response=await handle(new Request(url,{method:request.method(),headers,...(body?{body}:{} )}),env);
   if(request.method()==='PUT')writes.push(request.postData());
   return route.fulfill({status:response.status,headers:Object.fromEntries(response.headers),body:Buffer.from(await response.arrayBuffer())});
  }
  const key=Object.keys(domains).find(k=>domains[k]===url.hostname);
  const base=key==='run'?resolve(root,'dist/run'):['hub','desk'].includes(key)?resolve(root,`dist/apps/apps.${key}`):null;
  if(!base)return route.abort();let path=resolve(base,'.'+decodeURIComponent(url.pathname));
  if(path!==base&&!path.startsWith(base+sep))return route.abort();
  if(existsSync(path)&&statSync(path).isDirectory())path=resolve(path,'index.html');
  if(!existsSync(path))return route.fulfill({status:404,body:'Missing compiled artifact'});
  const type={'.html':'text/html','.js':'text/javascript','.wasm':'application/wasm','.css':'text/css','.json':'application/json','.svg':'image/svg+xml'}[extname(path)]||'application/octet-stream';
  const csp=`default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' ${api}; worker-src 'self'; frame-src https://${domains.run}; frame-ancestors https://${domains.root} https://${domains.desk} https://${domains.hub}; object-src 'none'; base-uri 'self'`;
  return route.fulfill({contentType:type,body:readFileSync(path),headers:{'content-security-policy':csp}});
 });
 ctx.on('page',page=>page.on('pageerror',error=>errors.push(error.message)));
 return ctx;
}
async function panel(page){await page.locator('[data-account] summary').click();return page.locator('[data-account]');}
async function backup(cookie,key){const result=await invoke('GET',`/v1/sync/${key}`,undefined,cookie);assert.equal(result.status,200);return result.data;}
try{
 const owner=await register(),other=await register();
 const a=await context(owner),b=await context(owner),guest=await context();
 const signedOut=await guest.newPage();await signedOut.goto(`https://${domains.hub}/`);await panel(signedOut);
 await signedOut.getByText('Connectez-vous sur le site, puis actualisez la session ici.',{exact:false}).waitFor();
 assert.equal(await signedOut.locator('[data-backup-upload]').count(),0);await signedOut.getByRole('button',{name:'EN',exact:true}).click();await signedOut.getByRole('heading',{name:'Your space, your way.',exact:true}).waitFor();await signedOut.reload();await signedOut.getByRole('heading',{name:'Your space, your way.',exact:true}).waitFor();await guest.close();
 const first=await a.newPage(),second=await b.newPage();
 await first.goto(`https://${domains.hub}/`);const firstPanel=await panel(first);await firstPanel.getByText(owner.email,{exact:true}).waitFor();
 if(!published)assert.equal(writes.length,0,'no automatic upload');
 await first.locator('[data-widget="catalogue"] [data-tool="secure.hash-calculator"]').getByRole('button',{name:'Ajouter aux favoris'}).click();
 await first.locator('[data-widget="catalogue"] [data-tool="secure.hash-calculator"]').getByRole('link',{name:'Ouvrir',exact:true}).click();
 await first.waitForURL(/workspace.*tool=secure.hash-calculator/);await first.goto(`https://${domains.hub}/`);await panel(first);await firstPanel.getByText(owner.email,{exact:true}).waitFor();
 assert.ok(await first.evaluate(()=>Object.values(localStorage).some(value=>{try{return JSON.parse(value).launches?.['secure.hash-calculator']===1;}catch{return false;}})));
 await first.getByLabel('Mode',{exact:true}).selectOption('pro');await first.getByRole('button',{name:'Thème',exact:true}).click();
 await firstPanel.locator('[data-backup-upload]').click();await firstPanel.getByText('Sauvegarde enregistrée sur votre compte.',{exact:true}).waitFor();
 const initial=await backup(owner.cookie,'hub.layout.v1');assert.equal(initial.version,1);assert.deepEqual(initial.data.data.favorites,['secure.hash-calculator']);assert.deepEqual(initial.data.data.launches,{},'local usage statistics are excluded from account snapshots');
 assert.equal(initial.data.preferences.mode,'pro');assert.ok(!JSON.stringify(initial.data).includes(owner.cookie));
 await firstPanel.locator('[data-backup-download]').click();await firstPanel.locator('[data-backup-preview]').waitFor();await firstPanel.locator('[data-backup-confirm]').click();await firstPanel.getByText('Disposition restaurée sur cet appareil.',{exact:true}).waitFor();
 assert.ok(await first.evaluate(()=>Object.values(localStorage).some(value=>{try{return JSON.parse(value).launches?.['secure.hash-calculator']===1;}catch{return false;}})),'restoring account metadata preserves local usage counters');
 await second.goto(`https://${domains.hub}/`);const secondPanel=await panel(second);await secondPanel.getByText(owner.email,{exact:true}).waitFor();
 assert.equal(await second.locator('[data-widget="favorites"] [data-tool]').count(),0);
 await secondPanel.locator('[data-backup-download]').click();await secondPanel.locator('[data-backup-preview]').waitFor();
 assert.equal(await second.locator('[data-widget="favorites"] [data-tool]').count(),0,'preview does not change local state');
 await secondPanel.locator('[data-backup-confirm]').click();await secondPanel.getByText('Disposition restaurée sur cet appareil.',{exact:true}).waitFor();
 assert.equal(await second.locator('[data-widget="favorites"] [data-tool="secure.hash-calculator"]').count(),1);
 assert.equal(await second.getByLabel('Mode',{exact:true}).inputValue(),'pro');assert.equal(await second.locator('html').getAttribute('data-theme'),'dark');
 // A concurrent update must make the confirmation fail, never overwrite the newer copy.
 await firstPanel.locator('[data-backup-upload]').click();await firstPanel.locator('[data-backup-preview]').waitFor();
 assert.equal((await invoke('PUT','/v1/sync/hub.layout.v1',{version:1,data:initial.data},owner.cookie,owner.id)).status,200);
 await firstPanel.locator('[data-backup-confirm]').click();await firstPanel.getByRole('alert').filter({hasText:'Aucun écrasement effectué'}).waitFor();
 assert.equal((await backup(owner.cookie,'hub.layout.v1')).version,2);
 // Changing the cookie after preview must not write the old layout into the new account.
 await firstPanel.locator('[data-backup-upload]').click();await firstPanel.locator('[data-backup-preview]').waitFor();
 await addCookie(a,other);await firstPanel.locator('[data-backup-confirm]').click();
 await firstPanel.getByRole('alert').filter({hasText:'Session expirée ou compte changé'}).waitFor();
 assert.equal((await invoke('GET','/v1/sync/hub.layout.v1',undefined,other.cookie)).status,404);
 await addCookie(a,owner);
 results.push('Shared Rust cookie session and explicit metadata backup/restore across two contexts; preferences restored, local usage counters excluded from upload and preserved on restore.','Version conflict and account switch reject writes without replacing the newer backup.');
 const desk=await a.newPage();await desk.goto(`https://${domains.desk}/`);await desk.locator('[data-tool="secure.hash-calculator"]').waitFor();if(await desk.locator('.desk-window').count()===0)await desk.locator('[data-tool="secure.hash-calculator"]').click();
 const frame=desk.frameLocator('.desk-window iframe').first();await frame.getByLabel('Texte (si aucun fichier)',{exact:true}).fill('private fixture content');
 await frame.getByRole('button',{name:'Lancer',exact:true}).click();await frame.getByRole('button',{name:'Copier',exact:true}).first().waitFor();
 const before=await desk.locator('.desk-window iframe').first().elementHandle();const deskPanel=await panel(desk);await deskPanel.getByText(owner.email,{exact:true}).waitFor();
 await deskPanel.locator('[data-backup-upload]').click();await deskPanel.getByText('Sauvegarde enregistrée sur votre compte.',{exact:true}).waitFor();
 const saved=await backup(owner.cookie,'desk.layout.v1');assert.equal(saved.data.data.windows.length,1);assert.ok(!JSON.stringify(saved).includes('private fixture content'));
 assert.deepEqual(Object.keys(saved.data.data).sort(),['active','next','schema','spaces','windows']);
 await deskPanel.locator('[data-backup-download]').click();await deskPanel.locator('[data-backup-preview]').waitFor();
 assert.equal(await deskPanel.locator('[data-backup-confirm]').isDisabled(),true);assert.ok(await before.evaluate(el=>el.isConnected));
 assert.equal(await frame.getByLabel('Texte (si aucun fichier)',{exact:true}).inputValue(),'private fixture content');
 const restored=await b.newPage();await restored.goto(`https://${domains.desk}/`);const restoredPanel=await panel(restored);await restoredPanel.getByText(owner.email,{exact:true}).waitFor();
 await restoredPanel.locator('[data-backup-download]').click();await restoredPanel.locator('[data-backup-preview]').waitFor();await restoredPanel.locator('[data-backup-confirm]').click();
 await restoredPanel.getByText('Disposition restaurée sur cet appareil.',{exact:true}).waitFor();
 const empty=restored.frameLocator('.desk-window iframe').first();await empty.getByLabel('Texte (si aucun fichier)',{exact:true}).waitFor();
 assert.equal(await empty.getByLabel('Texte (si aucun fichier)',{exact:true}).inputValue(),'');
 results.push('Desk saves only layout metadata, restores an empty tool in a second context, and refuses restoration while a window is open.');
 assert.deepEqual(errors,[]);
 await Promise.all([a.close(),b.close()]);
 const output={date:new Date().toISOString(),mode:published?'public HTTPS':'compiled apps with isolated local API fixture',results,errors,fixtureAccounts:2};
 mkdirSync(resolve(root,'.kortex/qa'),{recursive:true});writeFileSync(resolve(root,`.kortex/qa/account-sync-${published?'public':'local'}.json`),JSON.stringify(output,null,2));
 console.log(JSON.stringify(output,null,2));
}finally{
 await browser.close();
 for(const user of users){const removed=await invoke('DELETE','/v1/me',user.proof,user.cookie);assert.equal(removed.status,200,'only the generated QA account must be cleaned up');}
 console.log('Generated QA accounts and their snapshots cleaned up; existing accounts unchanged.');
}
