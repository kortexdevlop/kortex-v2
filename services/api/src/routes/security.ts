import type { Ctx } from '../env.ts';
import { hashPassword, verifyPassword, sha256Hex, newSessionToken } from '../crypto.ts';
import { HttpError, readCookie, readJson, isRecord, json, cookie } from '../http.ts';
import { requireSession, SESSION_SECONDS } from '../session.ts';
import { checkAuthKey } from '../validate.ts';
import { rateLimit } from '../ratelimit.ts';

export async function securityBody(ctx: Ctx, max = 4000): Promise<Record<string, unknown>> {
  const b = await readJson(ctx.req, max);
  if (!isRecord(b)) throw new HttpError(400, 'invalid_json');
  return b;
}
export async function passwordProof(ctx: Ctx, b: Record<string, unknown>): Promise<{ id: string; hash: string }> {
  const id = await requireSession(ctx);
  await rateLimit(ctx, `security:${id}`, 15, 600);
  const key = checkAuthKey(b.auth_key, b.kdf);
  const user = await ctx.env.DB.prepare('SELECT password_hash FROM users WHERE id = ?1').bind(id).first<{password_hash: string}>();
  if (!user || !await verifyPassword(key, user.password_hash)) throw new HttpError(403, 'invalid_credentials');
  return { id, hash: user.password_hash };
}
export async function sessionId(ctx: Ctx): Promise<string> { return sha256Hex(readCookie(ctx.env, ctx.req) ?? ''); }

export async function listSessions(ctx: Ctx): Promise<Response> {
  const id = await requireSession(ctx), current = await sessionId(ctx);
  const rows = await ctx.env.DB.prepare('SELECT id, created_at, expires_at, user_agent FROM sessions WHERE user_id = ?1 AND expires_at > ?2 ORDER BY created_at DESC').bind(id, Date.now()).all<{id: string; created_at: number; expires_at: number; user_agent: string}>();
  // Database token hashes are never exposed as session identifiers.
  return json({ sessions: rows.results.map(({id, ...row}) => ({ ...row, current: id === current })) });
}
export async function revokeSessions(ctx: Ctx): Promise<Response> {
  const {id} = await passwordProof(ctx, await securityBody(ctx));
  await ctx.env.DB.prepare('DELETE FROM sessions WHERE user_id = ?1 AND id != ?2').bind(id, await sessionId(ctx)).run();
  return json({ok: true});
}
export async function changePassword(ctx: Ctx): Promise<Response> {
  const b = await securityBody(ctx), {id, hash} = await passwordProof(ctx, b);
  const key = checkAuthKey(b.new_auth_key, b.kdf);
  if (key === b.auth_key) throw new HttpError(400, 'password_unchanged');
  const next = await hashPassword(key, Number(ctx.env.PBKDF2_ITERATIONS ?? '5000'));
  const token = newSessionToken(), tokenHash = await sha256Hex(token), now = Date.now();
  const db = ctx.env.DB;
  // D1 batch is atomic. CAS and dependent predicates prevent concurrent rotations
  // from revoking the winning session or installing a losing password/session.
  const results = await db.batch([
    db.prepare('UPDATE users SET password_hash = ?3 WHERE id = ?1 AND password_hash = ?2').bind(id, hash, next),
    db.prepare('DELETE FROM sessions WHERE user_id = ?1 AND EXISTS (SELECT 1 FROM users WHERE id = ?1 AND password_hash = ?2)').bind(id, next),
    db.prepare('DELETE FROM devices WHERE user_id = ?1 AND EXISTS (SELECT 1 FROM users WHERE id = ?1 AND password_hash = ?2)').bind(id, next),
    db.prepare('DELETE FROM webauthn_challenges WHERE user_id = ?1 AND EXISTS (SELECT 1 FROM users WHERE id = ?1 AND password_hash = ?2)').bind(id, next),
    db.prepare('INSERT INTO sessions (id,user_id,created_at,expires_at,user_agent) SELECT ?1,?2,?3,?4,?5 WHERE EXISTS (SELECT 1 FROM users WHERE id = ?2 AND password_hash = ?6)').bind(tokenHash,id,now,now+SESSION_SECONDS*1000,ctx.req.headers.get('user-agent')?.slice(0,200) ?? null,next),
  ]);
  if (results[0]?.meta.changes !== 1) throw new HttpError(409, 'security_conflict');
  return json({ok: true}, {headers:{'Set-Cookie': cookie(ctx.env,token,SESSION_SECONDS)}});
}
