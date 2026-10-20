import { generateRegistrationOptions, verifyRegistrationResponse, generateAuthenticationOptions, verifyAuthenticationResponse } from '@simplewebauthn/server';
import type { RegistrationResponseJSON, AuthenticationResponseJSON } from '@simplewebauthn/server';
import type { Ctx } from '../env.ts';
import { HttpError, json, cookie } from '../http.ts';
import { rateLimit } from '../ratelimit.ts';
import { requireSession, createSession, SESSION_SECONDS } from '../session.ts';
import { passwordProof, securityBody, sessionId } from './security.ts';

const rp = (ctx: Ctx) => ({rpID: ctx.env.WEBAUTHN_RP_ID ?? 'localhost', origin: ctx.env.WEBAUTHN_ORIGIN ?? 'http://localhost:4321'});
type Challenge = {challenge: string; name: string; user_id: string | null};
async function save(ctx: Ctx, challenge: string, kind: string, user: string | null = null, name = ''): Promise<string> {
  const id = crypto.randomUUID();
  await ctx.env.DB.prepare('INSERT INTO webauthn_challenges (id,challenge,kind,user_id,session_id,name,expires_at) VALUES (?1,?2,?3,?4,?5,?6,?7)').bind(id,challenge,kind,user,user ? await sessionId(ctx) : null,name,Date.now()+300_000).run();
  return id;
}
async function consume(ctx: Ctx, id: unknown, kind: string, user: string | null = null): Promise<Challenge> {
  if (typeof id !== 'string' || id.length > 64) throw new HttpError(400,'invalid_challenge');
  const row = await ctx.env.DB.prepare('DELETE FROM webauthn_challenges WHERE id = ?1 AND kind = ?2 AND expires_at > ?3 AND (user_id IS NULL OR (user_id = ?4 AND session_id = ?5)) RETURNING challenge,name,user_id').bind(id,kind,Date.now(),user,user ? await sessionId(ctx) : null).first<Challenge>();
  if (!row) throw new HttpError(400,'invalid_challenge');
  return row;
}
export async function list(ctx: Ctx): Promise<Response> {
  const id = await requireSession(ctx);
  const rows = await ctx.env.DB.prepare('SELECT id,name,created_at,last_used FROM credentials WHERE user_id = ?1 ORDER BY created_at').bind(id).all();
  return json({passkeys: rows.results});
}
export async function remove(ctx: Ctx, params: Record<string,string>): Promise<Response> {
  const {id} = await passwordProof(ctx, await securityBody(ctx));
  await ctx.env.DB.prepare('DELETE FROM credentials WHERE id = ?1 AND user_id = ?2').bind(params.id,id).run();
  return json({ok:true});
}
export async function registrationOptions(ctx: Ctx): Promise<Response> {
  const b = await securityBody(ctx), {id} = await passwordProof(ctx,b);
  if (typeof b.name !== 'string' || !b.name.trim() || b.name.length > 80) throw new HttpError(400,'invalid_name');
  const user = await ctx.env.DB.prepare('SELECT email FROM users WHERE id = ?1').bind(id).first<{email:string}>();
  const existing = await ctx.env.DB.prepare('SELECT id FROM credentials WHERE user_id = ?1').bind(id).all<{id:string}>();
  if (existing.results.length >= 10) throw new HttpError(409,'passkey_limit');
  const options = await generateRegistrationOptions({rpName:'Kortex',rpID:rp(ctx).rpID,userName:user!.email,userID:new Uint8Array(new TextEncoder().encode(id)),attestationType:'none',supportedAlgorithmIDs:[-7],authenticatorSelection:{residentKey:'required',userVerification:'required'},excludeCredentials:existing.results,timeout:60_000});
  return json({options,challenge_id:await save(ctx,options.challenge,'register',id,b.name.trim())});
}
export async function registrationVerify(ctx: Ctx): Promise<Response> {
  const id = await requireSession(ctx);
  await rateLimit(ctx,`passkey:enroll:${id}`,15,600);
  const b = await securityBody(ctx,20_000), c = await consume(ctx,b.challenge_id,'register',id);
  let result;
  try { result = await verifyRegistrationResponse({response:b.response as RegistrationResponseJSON,expectedChallenge:c.challenge,expectedOrigin:rp(ctx).origin,expectedRPID:rp(ctx).rpID,requireUserVerification:true,supportedAlgorithmIDs:[-7]}); }
  catch { throw new HttpError(400,'invalid_passkey'); }
  if (!result.verified) throw new HttpError(400,'invalid_passkey');
  const key = result.registrationInfo.credential;
  const inserted = await ctx.env.DB.prepare('INSERT INTO credentials (id,user_id,public_key,counter,created_at,name,transports) SELECT ?1,?2,?3,?4,?5,?6,?7 WHERE (SELECT count(*) FROM credentials WHERE user_id = ?2) < 10 ON CONFLICT(id) DO NOTHING').bind(key.id,id,key.publicKey,key.counter,Date.now(),c.name,JSON.stringify(key.transports ?? [])).run();
  if (inserted.meta.changes !== 1) throw new HttpError(409,'passkey_limit');
  return json({ok:true},{status:201});
}
export async function authenticationOptions(ctx: Ctx): Promise<Response> {
  await rateLimit(ctx,`passkey:options:${ctx.ip}`,20,60);
  const options = await generateAuthenticationOptions({rpID:rp(ctx).rpID,userVerification:'required',timeout:60_000});
  return json({options,challenge_id:await save(ctx,options.challenge,'login')});
}
export async function authenticationVerify(ctx: Ctx): Promise<Response> {
  await rateLimit(ctx,`passkey:verify:${ctx.ip}`,20,60);
  const b = await securityBody(ctx,20_000), c = await consume(ctx,b.challenge_id,'login');
  const response = b.response as AuthenticationResponseJSON;
  if (!response || typeof response.id !== 'string') throw new HttpError(400,'invalid_passkey');
  const key = await ctx.env.DB.prepare('SELECT id,user_id,public_key,counter FROM credentials WHERE id = ?1').bind(response.id).first<{id:string;user_id:string;public_key:ArrayBuffer;counter:number}>();
  if (!key) throw new HttpError(400,'invalid_passkey');
  let result;
  try { result = await verifyAuthenticationResponse({response,expectedChallenge:c.challenge,expectedOrigin:rp(ctx).origin,expectedRPID:rp(ctx).rpID,requireUserVerification:true,credential:{id:key.id,publicKey:new Uint8Array(key.public_key),counter:key.counter}}); }
  catch { throw new HttpError(400,'invalid_passkey'); }
  if (!result.verified) throw new HttpError(400,'invalid_passkey');
  const changed = await ctx.env.DB.prepare('UPDATE credentials SET counter = ?3,last_used = ?4 WHERE id = ?1 AND counter = ?2').bind(key.id,key.counter,result.authenticationInfo.newCounter,Date.now()).run();
  if (changed.meta.changes !== 1) throw new HttpError(400,'invalid_passkey');
  const token = await createSession(ctx.env,key.user_id,ctx.req.headers.get('user-agent'));
  return json({ok:true},{headers:{'Set-Cookie':cookie(ctx.env,token,SESSION_SECONDS)}});
}
