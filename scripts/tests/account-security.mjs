// Real browser WebAuthn, using a virtual authenticator and only its own disposable account.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {readFileSync,writeFileSync,existsSync,unlinkSync} from 'node:fs';
import {randomUUID} from 'node:crypto';
import {deriveAuthKey} from '../../services/shared/src/auth.ts';
const {chromium}=createRequire(import.meta.url)('playwright');
const origin=process.env.KORTEX_SITE_ORIGIN || 'https://kortex.site';
const api=process.env.KORTEX_API_ORIGIN || 'https://api.kortex.site';
const qaFile=process.env.KORTEX_QA_FILE;
const qa=qaFile && existsSync(qaFile) ? JSON.parse(readFileSync(qaFile,'utf8')) : {email:`security-${randomUUID()}@example.invalid`,password:`Kortex-QA-${randomUUID()}!`};
assert.ok(/@example\.invalid$/.test(qa.email),'Security recipe requires a disposable @example.invalid account');
let registered=Boolean(qaFile && existsSync(qaFile)),erased=false,password=qa.password;
const browser=await chromium.launch({headless:true,args:process.env.KORTEX_QA_HOST_RULES ? [`--host-resolver-rules=${process.env.KORTEX_QA_HOST_RULES}`] : [],executablePath:process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
const context=await browser.newContext({serviceWorkers:'block'});
const page=await context.newPage();const errors=[];
page.on('pageerror',e=>errors.push(e.message));
const call=(method,path,data)=>context.request.fetch(`${api}${path}`,{method,headers:{Origin:origin},data});
const proof=async(p=password)=>({email:qa.email,auth_key:await deriveAuthKey(qa.email,p),kdf:1});
async function loginUI(p=password){
 await page.goto(`${origin}/compte`,{waitUntil:'networkidle'});
 await page.locator('[data-view=auth]').waitFor({state:'visible'});
 await page.locator('[data-security-password]').waitFor({state:'attached'});
 await page.locator('[data-form] [name=email]').fill(qa.email);
 await page.locator('[data-form] [name=password]').fill(p);
 await page.locator('[data-submit]').click();await page.locator('[data-view=me]').waitFor({state:'visible'});
}
try {
 if(!registered){const r=await call('POST','/v1/auth/register',await proof());assert.equal(r.status(),201);registered=true;await call('POST','/v1/auth/logout',{});}
 await loginUI();console.log('Password login OK; security panel present.');
 const oldCookies=await context.cookies();
 const cdp=await context.newCDPSession(page);
 await cdp.send('WebAuthn.enable');
 await cdp.send('WebAuthn.addVirtualAuthenticator',{options:{protocol:'ctap2',transport:'internal',hasResidentKey:true,hasUserVerification:true,isUserVerified:true,automaticPresenceSimulation:true}});
 await page.locator('[data-security-password]').fill(password);
 await page.locator('[data-passkey-form] [name=name]').fill('QA virtual passkey');
 const enrollment=page.waitForResponse(r=>r.url().endsWith('/v1/me/passkeys/verify'));
 await page.locator('[data-add-passkey]').click();assert.equal((await enrollment).status(),201);console.log('WebAuthn enrollment OK.');
 await page.locator('[data-passkeys]').getByText(/QA virtual passkey/).waitFor();
 await page.locator('[data-logout]').click();await page.locator('[data-view=auth]').waitFor({state:'visible'});
 const signed=page.waitForResponse(r=>r.url().endsWith('/v1/auth/passkey/verify'));
 await page.locator('[data-passkey-login]').click();assert.equal((await signed).status(),200);console.log('WebAuthn sign-in OK.');
 await page.locator('[data-view=me]').waitFor({state:'visible'});
 assert.equal(await page.locator('[data-email]').textContent(),qa.email);
 // A second independent session must be invalidated when the password changes.
 const other=await browser.newContext();await other.addCookies(oldCookies);
 const secondLogin=await other.request.post(`${api}/v1/auth/login`,{headers:{Origin:origin},data:await proof()});assert.equal(secondLogin.status(),200);
 const next=`Kortex-QA-rotated-${randomUUID()}!`;
 await page.locator('[data-security-password]').fill(password);
 await page.locator('[data-password-form] [name=next]').fill(next);
 await page.locator('[data-password-form] [name=repeat]').fill(next);
 const rotation=page.waitForResponse(r=>r.url().endsWith('/v1/me/password'));
 await page.locator('[data-password-form] button').click();assert.equal((await rotation).status(),200);console.log('Password rotation OK.');
 const previous=password;password=next;
 if(qaFile) writeFileSync(qaFile,JSON.stringify({...qa,password}),{mode:0o600});
 assert.equal((await other.request.get(`${api}/v1/me`,{headers:{Origin:origin}})).status(),401);await other.close();
 assert.equal((await call('POST','/v1/auth/login',await proof(previous))).status(),401);
 const sessions=await (await call('GET','/v1/me/sessions')).json();assert.equal(sessions.sessions.length,1);assert.equal(sessions.sessions[0].current,true);
 assert.equal((await call('GET','/v1/me')).status(),200);
 const exported=await (await call('GET','/v1/me/export')).json();assert.equal(exported.passkeys.length,1);assert.ok(!JSON.stringify(exported).includes('public_key'));
 // Passkeys survive password rotation and can still sign in.
 await page.locator('[data-logout]').click();await page.locator('[data-view=auth]').waitFor({state:'visible'});
 await page.locator('[data-passkey-login]').click();await page.locator('[data-view=me]').waitFor({state:'visible'});
 await page.locator('[data-passkeys] button').waitFor();
 await page.locator('[data-security-password]').fill(password);
 const removed=page.waitForResponse(r=>r.request().method()==='DELETE'&&r.url().includes('/v1/me/passkeys/'));
 await page.locator('[data-passkeys] button').click();assert.equal((await removed).status(),200);
 // Create another session, revoke it from the UI, and retain the current session.
 const sibling=await browser.newContext();assert.equal((await sibling.request.post(`${api}/v1/auth/login`,{headers:{Origin:origin},data:await proof()})).status(),200);
 await page.locator('[data-security-password]').fill(password);
 const revoked=page.waitForResponse(r=>r.url().endsWith('/v1/me/sessions/revoke'));
 await page.locator('[data-revoke-sessions]').click();assert.equal((await revoked).status(),200);
 assert.equal((await sibling.request.get(`${api}/v1/me`,{headers:{Origin:origin}})).status(),401);await sibling.close();
 await page.goto(`${origin}/en/account`,{waitUntil:'networkidle'});await page.getByRole('heading',{name:'Account security',exact:true}).waitFor();
 assert.equal(await page.locator('[data-security-password]').inputValue(),'');
 await page.locator('[data-del-open]').click();await page.locator('[data-del-form] [name=password]').fill(password);
 const deletion=page.waitForResponse(r=>r.request().method()==='DELETE'&&r.url().endsWith('/v1/me'));
 await page.locator('[data-del-form] button[type=submit]').click();assert.equal((await deletion).status(),200);erased=true;
 assert.deepEqual(errors,[]);
 console.log('Security OK: real WebAuthn enrollment/login, password rotation, old-session rejection, passkey retention/revocation, other-session revocation, FR/EN, export, disposable account erased.');
} finally {
 try {
  if(registered&&!erased){assert.equal((await call('POST','/v1/auth/login',await proof())).status(),200,'QA cleanup login');assert.equal((await call('DELETE','/v1/me',await proof())).status(),200,'QA cleanup delete');erased=true;}
  if(erased&&qaFile&&existsSync(qaFile)) unlinkSync(qaFile);
 }finally{await browser.close();}
}
