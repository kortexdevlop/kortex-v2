import {test} from 'node:test';
import assert from 'node:assert/strict';
import {makeEnv,signup,call,creds} from './helpers.ts';
import {newSessionToken,sha256Hex} from '../src/crypto.ts';
import {verifyRegistrationGuard} from '../src/turnstile.ts';

// Small WebAuthn fixture encoder, independent of the verifier implementation.
const bytes=(...values:Uint8Array[])=>Buffer.concat(values);
const cbor=(value:any):Buffer=>{
 const head=(major:number,n:number)=>n<24?Buffer.from([major*32+n]):n<256?Buffer.from([major*32+24,n]):Buffer.from([major*32+25,n>>8,n&255]);
 if(typeof value==='number') return head(value<0?1:0,value<0?-1-value:value);
 if(typeof value==='string'){const b=Buffer.from(value);return bytes(head(3,b.length),b);}
 if(value instanceof Uint8Array) return bytes(head(2,value.length),value);
 const entries=value instanceof Map?[...value]:Object.entries(value);return bytes(head(5,entries.length),...entries.flatMap(([k,v]:any)=>[cbor(k),cbor(v)]));
};
const b64=(v:Uint8Array)=>Buffer.from(v).toString('base64url');
const hash=async(s:string)=>new Uint8Array(await crypto.subtle.digest('SHA-256',Buffer.from(s)));
async function authenticator(){
 const pair=await crypto.subtle.generateKey({name:'ECDSA',namedCurve:'P-256'},true,['sign','verify']) as CryptoKeyPair;
 const pub=await crypto.subtle.exportKey('jwk',pair.publicKey) as JsonWebKey;
 const cose=cbor(new Map<any,any>([[1,2],[3,-7],[-1,1],[-2,Buffer.from(pub.x!,'base64url')],[-3,Buffer.from(pub.y!,'base64url')]]));
 const id=crypto.getRandomValues(new Uint8Array(32));
 const client=(challenge:string,type:string,origin='https://kortex.site')=>Buffer.from(JSON.stringify({type,challenge,origin,crossOrigin:false}));
 const authData=async(flags:number,counter:number)=>bytes(await hash('kortex.site'),Buffer.from([flags]),Buffer.from([0,0,0,counter]));
 return {
  id:b64(id),
  async registration(challenge:string,origin='https://kortex.site',uv=true){
   const data=bytes(await authData(uv?0x45:0x41,0),new Uint8Array(16),Buffer.from([0,id.length]),id,cose);
   return {id:b64(id),rawId:b64(id),type:'public-key',response:{clientDataJSON:b64(client(challenge,'webauthn.create',origin)),attestationObject:b64(cbor({fmt:'none',attStmt:{},authData:data})),transports:['internal']},clientExtensionResults:{},authenticatorAttachment:'platform'};
  },
  async assertion(challenge:string,counter=1,origin='https://kortex.site',uv=true){
   const c=client(challenge,'webauthn.get',origin),a=await authData(uv?5:1,counter);
   const sig=new Uint8Array(await crypto.subtle.sign({name:'ECDSA',hash:'SHA-256'},pair.privateKey,bytes(a,await crypto.subtle.digest('SHA-256',c).then(x=>new Uint8Array(x)))));
   const integer=(b:Uint8Array)=>{let v=Buffer.from(b);while(v.length>1&&v[0]===0)v=v.subarray(1);if(v[0]!&128)v=bytes(Buffer.from([0]),v);return bytes(Buffer.from([2,v.length]),v);};
   const rs=bytes(integer(sig.subarray(0,32)),integer(sig.subarray(32)));
   return {id:b64(id),rawId:b64(id),type:'public-key',response:{clientDataJSON:b64(c),authenticatorData:b64(a),signature:b64(bytes(Buffer.from([0x30,rs.length]),rs)),userHandle:null},clientExtensionResults:{}};
  }
 };
}
test('passkeys: password/session binding, signed login, replay, wrong origin/UV/signature, revoke and export',async()=>{
 const {env,raw}=makeEnv();const s=await signup(env);const proof=await creds('a@example.com','correct horse battery');const key=await authenticator();
 const enroll=()=>call(env,'POST','/v1/me/passkeys/options',{...proof,name:'QA key'},{cookie:s.cookie});
 assert.equal((await call(env,'POST','/v1/me/passkeys/options',{...proof,name:'Key'})).status,401);
 assert.equal((await call(env,'POST','/v1/me/passkeys/options',{...await creds('a@example.com','bad password'),name:'Key'},{cookie:s.cookie})).status,403);
 let p=await enroll();const other=await call(env,'POST','/v1/auth/login',proof);const cookie2=other.headers.get('set-cookie')!.split(';')[0];
 const registered=await key.registration(p.data.options.challenge);
 assert.equal((await call(env,'POST','/v1/me/passkeys/verify',{challenge_id:p.data.challenge_id,response:registered},{cookie:cookie2})).status,400);
 assert.equal((await call(env,'POST','/v1/me/passkeys/verify',{challenge_id:p.data.challenge_id,response:registered},{cookie:s.cookie})).status,201);
 assert.equal((await call(env,'POST','/v1/me/passkeys/verify',{challenge_id:p.data.challenge_id,response:registered},{cookie:s.cookie})).status,400);
 for(const opts of [{origin:'https://evil.test',uv:true},{origin:'https://kortex.site',uv:false}]){
  p=await enroll();assert.equal((await call(env,'POST','/v1/me/passkeys/verify',{challenge_id:p.data.challenge_id,response:await key.registration(p.data.options.challenge,opts.origin,opts.uv)},{cookie:s.cookie})).status,400);
 }
 const loginOptions=()=>call(env,'POST','/v1/auth/passkey/options',{});
 for(const [origin,uv,tamper] of [['https://evil.test',true,false],['https://kortex.site',false,false],['https://kortex.site',true,true]] as const){
  const o=await loginOptions(),response=await key.assertion(o.data.options.challenge,1,origin,uv);
  if(tamper) response.response.signature=b64(new Uint8Array(64));
  assert.equal((await call(env,'POST','/v1/auth/passkey/verify',{challenge_id:o.data.challenge_id,response})).status,400);
 }
 const o=await loginOptions(),response=await key.assertion(o.data.options.challenge);
 const signed=await call(env,'POST','/v1/auth/passkey/verify',{challenge_id:o.data.challenge_id,response});assert.equal(signed.status,200);assert.ok(signed.headers.get('set-cookie'));
 assert.equal((await call(env,'POST','/v1/auth/passkey/verify',{challenge_id:o.data.challenge_id,response})).status,400);
 const replay=await loginOptions();assert.equal((await call(env,'POST','/v1/auth/passkey/verify',{challenge_id:replay.data.challenge_id,response:await key.assertion(replay.data.options.challenge)})).status,400);
 const exported=await call(env,'GET','/v1/me/export',undefined,{cookie:s.cookie});assert.equal(exported.data.passkeys.length,1);assert.ok(!JSON.stringify(exported.data).includes('public_key'));
 assert.equal((await call(env,'DELETE',`/v1/me/passkeys/${key.id}`,proof,{cookie:s.cookie})).status,200);
 assert.equal(raw.prepare('SELECT count(*) AS n FROM credentials').get()!.n,0);
 raw.close();
});
test('expired challenges and cross-user key deletion are rejected',async()=>{
 const {env,raw}=makeEnv();const a=await signup(env),b=await signup(env,'b@example.com');const proof=await creds('a@example.com','correct horse battery');
 const o=await call(env,'POST','/v1/me/passkeys/options',{...proof,name:'Key'},{cookie:a.cookie});
 raw.prepare('UPDATE webauthn_challenges SET expires_at=0').run();
 assert.equal((await call(env,'POST','/v1/me/passkeys/verify',{challenge_id:o.data.challenge_id,response:{}},{cookie:a.cookie})).status,400);
 raw.prepare("INSERT INTO credentials (id,user_id,public_key,created_at) VALUES ('owned',?,X'00',0)").run(a.data.id);
 await call(env,'DELETE','/v1/me/passkeys/owned',await creds('b@example.com','correct horse battery'),{cookie:b.cookie});
 assert.equal(raw.prepare('SELECT count(*) AS n FROM credentials').get()!.n,1);raw.close();
});
test('password rotation revokes browser/device tokens, keeps passkeys, issues a new cookie and rejects old proof',async()=>{
 const {env,raw}=makeEnv();const s=await signup(env);const old=await creds('a@example.com','correct horse battery');
 const login=await call(env,'POST','/v1/auth/login',old);const second=login.headers.get('set-cookie')!.split(';')[0];
 const device=newSessionToken();raw.prepare("INSERT INTO devices (id,user_id,name,platform,token_hash,created_at,last_seen) VALUES ('d',?,'QA','test',?,0,0)").run(s.data.id,await sha256Hex(device));
 raw.prepare("INSERT INTO credentials (id,user_id,public_key,created_at) VALUES ('key',?,X'00',0)").run(s.data.id);
 const next=await creds('a@example.com','a stronger new password');
 const changed=await call(env,'POST','/v1/me/password',{...old,new_auth_key:next.auth_key},{cookie:s.cookie});assert.equal(changed.status,200);
 for(const c of [s.cookie,second]) assert.equal((await call(env,'GET','/v1/me',undefined,{cookie:c})).status,401);
 assert.equal((await call(env,'GET','/v1/me',undefined,{headers:{Authorization:`Bearer ${device}`}})).status,401);
 assert.equal(raw.prepare('SELECT count(*) AS n FROM credentials').get()!.n,1);
 assert.equal((await call(env,'POST','/v1/auth/login',old)).status,401);assert.equal((await call(env,'POST','/v1/auth/login',next)).status,200);
 const cookie=changed.headers.get('set-cookie')!.split(';')[0];
 const sessions=await call(env,'GET','/v1/me/sessions',undefined,{cookie});assert.equal(sessions.data.sessions.length,2);assert.equal(sessions.data.sessions.filter((s:any)=>s.current).length,1);assert.ok(!JSON.stringify(sessions.data).includes('"id"'));
 assert.equal((await call(env,'POST','/v1/me/sessions/revoke',old,{cookie})).status,403);
 assert.equal((await call(env,'POST','/v1/me/sessions/revoke',next,{cookie})).status,200);assert.equal(raw.prepare('SELECT count(*) AS n FROM sessions').get()!.n,1);raw.close();
});
test('Turnstile fails closed: missing secret/token, hostname/action mismatch and provider failure',async()=>{
 const {env,raw}=makeEnv({TURNSTILE_REQUIRED:'true'});
 const ctx={env,req:new Request('https://api.kortex.site'),url:new URL('https://api.kortex.site'),ip:'127.0.0.1'};
 await assert.rejects(()=>verifyRegistrationGuard(ctx,'token'),/registration_unavailable/);
 env.TURNSTILE_SECRET_KEY='test-only';await assert.rejects(()=>verifyRegistrationGuard(ctx,''),/challenge_required/);
 const original=globalThis.fetch;
 try {
  for(const data of [{success:false},{success:true,hostname:'evil.test',action:'register'},{success:true,hostname:'kortex.site',action:'login'}]){
   globalThis.fetch=async()=>Response.json(data);await assert.rejects(()=>verifyRegistrationGuard(ctx,'token'),/challenge_required/);
  }
  globalThis.fetch=async()=>{throw new Error('offline');};await assert.rejects(()=>verifyRegistrationGuard(ctx,'token'),/registration_unavailable/);
  globalThis.fetch=async()=>Response.json({success:true,hostname:'kortex.site',action:'register'});await verifyRegistrationGuard(ctx,'token');
 }finally{globalThis.fetch=original;raw.close();}
});
