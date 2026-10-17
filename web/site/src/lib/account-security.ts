import { startRegistration } from '@simplewebauthn/browser';
import { deriveAuthKey, checkPasswordPolicy } from '../../../../services/shared/src/auth';

type Request = (method: string,path: string,body?: unknown) => Promise<{ok:boolean;data:any}>;
export function setupSecurity(root: HTMLElement, request: Request, passwordMessage: (code:string)=>string) {
 const panel = root.querySelector<HTMLElement>('[data-security]')!;
 const T = JSON.parse(panel.dataset.t!);
 const $ = <E extends HTMLElement>(sel:string) => panel.querySelector<E>(sel)!;
 const text = (tag:string,value:string) => Object.assign(document.createElement(tag),{textContent:value});
 const status = (value:string) => { $('[data-security-status]').textContent=value; };
 let busy=false;
 async function proof() {
  const field=$<HTMLInputElement>('[data-security-password]');
  if (!field.value) { field.focus(); throw new Error(T.proofHint); }
  return {auth_key:await deriveAuthKey(root.querySelector('[data-email]')!.textContent!,field.value),kdf:1};
 }
 async function checked(method:string,path:string,body?:unknown) {
  const r=await request(method,path,body);
  if(!r.ok) throw new Error(T.errors[r.data?.error] ?? T.errors.other);
  return r.data;
 }
 async function action(fn:()=>Promise<void>) {
  if(busy) return;
  busy=true; panel.querySelectorAll<HTMLButtonElement>('button').forEach(b=>b.disabled=true);status(T.working);
  try { await fn(); $<HTMLInputElement>('[data-security-password]').value=''; status(T.saved); await load(); }
  catch(e) { status(e instanceof Error ? e.message : T.cancelled); }
  finally { busy=false;panel.querySelectorAll<HTMLButtonElement>('button').forEach(b=>b.disabled=false); }
 }
 async function load() {
  const [keys,sessions]=await Promise.all([request('GET','/v1/me/passkeys'),request('GET','/v1/me/sessions')]);
  const list=$('[data-passkeys]'), slist=$('[data-sessions]'); list.replaceChildren();slist.replaceChildren();
  if(!keys.ok || !sessions.ok) { status(T.loadError); return; }
  if(!keys.data.passkeys.length) list.append(text('li',T.empty));
  for(const key of keys.data.passkeys) {
   const li=text('li','');li.className='k-file-row';
   const label=text('span',`${key.name} · ${T.created} ${new Date(key.created_at).toLocaleDateString()}${key.last_used ? ` · ${T.used} ${new Date(key.last_used).toLocaleDateString()}` : ''}`);
   const btn=text('button',T.remove) as HTMLButtonElement;btn.type='button';btn.className='k-btn';btn.dataset.variant='ghost';btn.disabled=busy;
   btn.addEventListener('click',()=>void action(async()=>{await checked('DELETE',`/v1/me/passkeys/${encodeURIComponent(key.id)}`,await proof());}));li.append(label,btn);list.append(li);
  }
  for(const session of sessions.data.sessions) {
   const li=text('li',`${session.current ? T.current : (session.user_agent || T.unknown)} · ${new Date(session.created_at).toLocaleString()}`);li.className='k-file-row';slist.append(li);
  }
 }
 $<HTMLFormElement>('[data-passkey-form]').addEventListener('submit',e=>{e.preventDefault();void action(async()=>{
  if(!window.PublicKeyCredential) throw new Error(T.unsupported);
  const f=e.currentTarget as HTMLFormElement;
  const pending=await checked('POST','/v1/me/passkeys/options',{...await proof(),name:(f.elements.namedItem('name') as HTMLInputElement).value});
  let response;try { response=await startRegistration({optionsJSON:pending.options}); } catch { throw new Error(T.cancelled); }
  await checked('POST','/v1/me/passkeys/verify',{challenge_id:pending.challenge_id,response});f.reset();
 });});
 $('[data-revoke-sessions]').addEventListener('click',()=>void action(async()=>{await checked('POST','/v1/me/sessions/revoke',await proof());}));
 $<HTMLFormElement>('[data-password-form]').addEventListener('submit',e=>{e.preventDefault();const f=e.currentTarget as HTMLFormElement;void action(async()=>{
  const next=(f.elements.namedItem('next') as HTMLInputElement).value,repeat=(f.elements.namedItem('repeat') as HTMLInputElement).value;
  if(next!==repeat) throw new Error(T.mismatch);
  const policy=checkPasswordPolicy(next);if(policy!=='ok') throw new Error(passwordMessage(policy));
  await checked('POST','/v1/me/password',{...await proof(),new_auth_key:await deriveAuthKey(root.querySelector('[data-email]')!.textContent!,next)});f.reset();
 });});
 return {load,reset:()=>{panel.querySelectorAll<HTMLInputElement>('input').forEach(input=>input.value='');status('');}};
}
