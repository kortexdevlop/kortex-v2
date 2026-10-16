import type { Ctx, Env } from './env.ts';
import { newSessionToken, sha256Hex } from './crypto.ts';
import { HttpError, readCookie } from './http.ts';

export const SESSION_SECONDS = 30 * 24 * 3600;

export async function createSession(env: Env, userId: string, userAgent: string | null, now = Date.now()): Promise<string> {
  const token = newSessionToken();
  const id = await sha256Hex(token);
  await env.DB
    .prepare('INSERT INTO sessions (id, user_id, created_at, expires_at, user_agent) VALUES (?1, ?2, ?3, ?4, ?5)')
    .bind(id, userId, now, now + SESSION_SECONDS * 1000, userAgent?.slice(0, 200) ?? null)
    .run();
  return token;
}

function bearer(req: Request): string | null {
  const h = req.headers.get('authorization') ?? '';
  return h.startsWith('Bearer ') ? h.slice(7).trim() || null : null;
}

/** Resolves a device token (Authorization: Bearer) to its user, or throws 401. */
async function requireDevice(ctx: Ctx, token: string, now: number): Promise<string> {
  const row = await ctx.env.DB
    .prepare('SELECT id, user_id FROM devices WHERE token_hash = ?1')
    .bind(await sha256Hex(token))
    .first<{ id: string; user_id: string }>();
  if (!row) throw new HttpError(401, 'unauthenticated');
  await ctx.env.DB.prepare('UPDATE devices SET last_seen = ?2 WHERE id = ?1').bind(row.id, now).run();
  ctx.userId = row.user_id;
  ctx.deviceId = row.id;
  return row.user_id;
}

/** Cookie session OR device token. Use `requireSession` for actions a device must not perform. */
export async function requireUser(ctx: Ctx, now = Date.now()): Promise<string> {
  const dev = bearer(ctx.req);
  if (dev) return requireDevice(ctx, dev, now);
  return requireSession(ctx, now);
}

/** Resolves the cookie to a user id, or throws 401. Expired sessions are removed. */
export async function requireSession(ctx: Ctx, now = Date.now()): Promise<string> {
  const token = readCookie(ctx.env, ctx.req);
  if (!token) throw new HttpError(401, 'unauthenticated');
  const id = await sha256Hex(token);
  const row = await ctx.env.DB
    .prepare('SELECT user_id, expires_at FROM sessions WHERE id = ?1')
    .bind(id)
    .first<{ user_id: string; expires_at: number }>();
  if (!row) throw new HttpError(401, 'unauthenticated');
  if (row.expires_at <= now) {
    await ctx.env.DB.prepare('DELETE FROM sessions WHERE id = ?1').bind(id).run();
    throw new HttpError(401, 'unauthenticated');
  }
  ctx.userId = row.user_id;
  return row.user_id;
}

export async function destroySession(ctx: Ctx): Promise<void> {
  const token = readCookie(ctx.env, ctx.req);
  if (token) await ctx.env.DB.prepare('DELETE FROM sessions WHERE id = ?1').bind(await sha256Hex(token)).run();
}
