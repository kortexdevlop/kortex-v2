import { passwordProof } from './security.ts';
import { verifyRegistrationGuard } from '../turnstile.ts';
import type { Ctx } from '../env.ts';
import { burnVerification, hashPassword, verifyPassword } from '../crypto.ts';
import { HttpError, cookie, errorBody, isRecord, json, readJson } from '../http.ts';
import { rateLimit } from '../ratelimit.ts';
import { SESSION_SECONDS, createSession, destroySession, requireUser } from '../session.ts';
import { checkAuthKey, normalizeEmail } from '../validate.ts';

const iterations = (ctx: Ctx): number => Number(ctx.env.PBKDF2_ITERATIONS ?? '5000');

async function body(ctx: Ctx): Promise<Record<string, unknown>> {
  const b = await readJson(ctx.req, 4_000);
  if (!isRecord(b)) throw new HttpError(400, 'invalid_json');
  return b;
}

export async function register(ctx: Ctx): Promise<Response> {
  await rateLimit(ctx, `register:${ctx.ip}`, 5, 600);
  const b = await body(ctx);
  await verifyRegistrationGuard(ctx,b.turnstile_token);
  const email = normalizeEmail(b.email);
  const authKey = checkAuthKey(b.auth_key, b.kdf);
  const id = crypto.randomUUID();
  const hash = await hashPassword(authKey, iterations(ctx));
  try {
    await ctx.env.DB.prepare('INSERT INTO users (id, email, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)').bind(id, email, hash, Date.now()).run();
  } catch (error) {
    if (error instanceof Error && /UNIQUE constraint failed: users\.email/i.test(error.message)) throw new HttpError(409, 'email_taken');
    throw error;
  }
  const token = await createSession(ctx.env, id, ctx.req.headers.get('user-agent'));
  return json({ id, email }, { status: 201, headers: { 'Set-Cookie': cookie(ctx.env, token, SESSION_SECONDS) } });
}

export async function login(ctx: Ctx): Promise<Response> {
  await rateLimit(ctx, `login:ip:${ctx.ip}`, 20, 60);
  const b = await body(ctx);
  const email = normalizeEmail(b.email);
  const authKey = checkAuthKey(b.auth_key, b.kdf);
  await rateLimit(ctx, `login:email:${email}`, 10, 600);
  const user = await ctx.env.DB.prepare('SELECT id, password_hash FROM users WHERE email = ?1').bind(email).first<{ id: string; password_hash: string }>();
  const ok = user ? await verifyPassword(authKey, user.password_hash) : (await burnVerification(authKey, iterations(ctx)), false);
  if (!user || !ok) return json(errorBody('invalid_credentials'), { status: 401 });
  const token = await createSession(ctx.env, user.id, ctx.req.headers.get('user-agent'));
  return json({ id: user.id, email }, { headers: { 'Set-Cookie': cookie(ctx.env, token, SESSION_SECONDS) } });
}

export async function logout(ctx: Ctx): Promise<Response> {
  await destroySession(ctx);
  return json({ ok: true }, { headers: { 'Set-Cookie': cookie(ctx.env, '', 0) } });
}

export async function me(ctx: Ctx): Promise<Response> {
  const id = await requireUser(ctx);
  const u = await ctx.env.DB.prepare('SELECT id, email, created_at FROM users WHERE id = ?1').bind(id).first();
  return json(u);
}

/** GDPR erasure: requires the credentials again, then deletes everything (cascade). */
export async function deleteMe(ctx: Ctx): Promise<Response> {
  const {id} = await passwordProof(ctx, await body(ctx));
  await ctx.env.DB.prepare('DELETE FROM users WHERE id = ?1').bind(id).run();
  return json({ ok: true }, { headers: { 'Set-Cookie': cookie(ctx.env, '', 0) } });
}

/** GDPR portability: everything we hold about the user, as JSON (no password hash). */
export async function exportMe(ctx: Ctx): Promise<Response> {
  const id = await requireUser(ctx);
  const db = ctx.env.DB;
  const [user, settings, sync, usage, devices, installs, passkeys, sessions] = await Promise.all([
    db.prepare('SELECT id, email, created_at FROM users WHERE id = ?1').bind(id).first(),
    db.prepare('SELECT data, updated_at FROM settings WHERE user_id = ?1').bind(id).first(),
    db.prepare('SELECT key, version, data, updated_at FROM sync_items WHERE user_id = ?1').bind(id).all(),
    db.prepare('SELECT day, tool, count FROM usage_daily WHERE user_id = ?1').bind(id).all(),
    db.prepare('SELECT id, name, platform, created_at, last_seen FROM devices WHERE user_id = ?1').bind(id).all(),
    db.prepare('SELECT device_id, tool, version, target, updated_at FROM installs WHERE user_id = ?1').bind(id).all(),
    db.prepare('SELECT name,created_at,last_used FROM credentials WHERE user_id = ?1').bind(id).all(),
    db.prepare('SELECT created_at,expires_at,user_agent FROM sessions WHERE user_id = ?1').bind(id).all(),
  ]);
  return json({ user, settings, sync: sync.results, usage: usage.results, devices: devices.results, installs: installs.results, passkeys: passkeys.results, sessions: sessions.results }, { headers: { 'Content-Disposition': 'attachment; filename="kortex-export.json"' } });
}
