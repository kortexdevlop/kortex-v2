import type { Ctx } from '../env.ts';
import { newSessionToken, burnVerification, sha256Hex, verifyPassword } from '../crypto.ts';
import { HttpError, errorBody, isRecord, json, readJson } from '../http.ts';
import { rateLimit } from '../ratelimit.ts';
import { requireSession, requireUser } from '../session.ts';
import { checkAuthKey, checkTool, normalizeEmail } from '../validate.ts';

const MAX_DEVICES = 20;
const TARGET = /^[a-z0-9_]+(-[a-z0-9_]+){1,3}$/;
const VERSION = /^\d{1,6}\.\d{1,6}\.\d{1,6}$/;
const iterations = (ctx: Ctx): number => Number(ctx.env.PBKDF2_ITERATIONS ?? '5000');

async function body(ctx: Ctx, max = 4_000): Promise<Record<string, unknown>> {
  const b = await readJson(ctx.req, max);
  if (!isRecord(b)) throw new HttpError(400, 'invalid_json');
  return b;
}

const clean = (v: unknown, max: number, fallback: string): string =>
  typeof v === 'string' && v.trim() ? v.trim().replace(/[\u0000-\u001f]/g, '').slice(0, max) : fallback;

/** CLI / desktop login: credentials in, a long-lived device token out (shown once, stored hashed). */
export async function createDevice(ctx: Ctx): Promise<Response> {
  await rateLimit(ctx, `device:ip:${ctx.ip}`, 20, 60);
  const b = await body(ctx);
  const email = normalizeEmail(b.email);
  const authKey = checkAuthKey(b.auth_key, b.kdf);
  await rateLimit(ctx, `login:email:${email}`, 10, 600); // shares the login budget: no side door for guessing
  const user = await ctx.env.DB.prepare('SELECT id, password_hash FROM users WHERE email = ?1').bind(email).first<{ id: string; password_hash: string }>();
  const ok = user ? await verifyPassword(authKey, user.password_hash) : (await burnVerification(authKey, iterations(ctx)), false);
  if (!user || !ok) return json(errorBody('invalid_credentials'), { status: 401 });
  const count = await ctx.env.DB.prepare('SELECT COUNT(*) AS n FROM devices WHERE user_id = ?1').bind(user.id).first<{ n: number }>();
  if ((count?.n ?? 0) >= MAX_DEVICES) throw new HttpError(409, 'too_many_devices');
  const token = `kdt_${newSessionToken()}`;
  const id = crypto.randomUUID();
  const now = Date.now();
  await ctx.env.DB
    .prepare('INSERT INTO devices (id, user_id, name, platform, token_hash, created_at, last_seen) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)')
    .bind(id, user.id, clean(b.name, 60, 'Appareil'), clean(b.platform, 40, ''), await sha256Hex(token), now)
    .run();
  return json({ device_id: id, token, email }, { status: 201 });
}

export async function listDevices(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  const r = await ctx.env.DB.prepare('SELECT id, name, platform, created_at, last_seen FROM devices WHERE user_id = ?1 ORDER BY created_at').bind(uid).all<{ id: string }>();
  return json({ current: ctx.deviceId ?? null, devices: r.results });
}

/** A device can revoke itself (`self`); revoking others needs the browser session. */
export async function deleteDevice(ctx: Ctx, p: Record<string, string>): Promise<Response> {
  const id = p.id ?? '';
  if (id === 'self') {
    await requireUser(ctx);
    if (!ctx.deviceId) throw new HttpError(400, 'device_required');
    await ctx.env.DB.prepare('DELETE FROM devices WHERE id = ?1').bind(ctx.deviceId).run();
    return json({ ok: true });
  }
  const uid = await requireSession(ctx);
  const r = await ctx.env.DB.prepare('DELETE FROM devices WHERE id = ?1 AND user_id = ?2').bind(id, uid).run();
  if (r.meta.changes === 0) return json(errorBody('not_found'), { status: 404 });
  return json({ ok: true });
}

/** Re-checks the password (as a derived key) before a sensitive action, e.g. installing a tool. */
export async function confirm(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  await rateLimit(ctx, `confirm:${uid}`, 10, 600);
  const b = await body(ctx);
  const authKey = checkAuthKey(b.auth_key, b.kdf);
  const row = await ctx.env.DB.prepare('SELECT password_hash FROM users WHERE id = ?1').bind(uid).first<{ password_hash: string }>();
  if (!row || !(await verifyPassword(authKey, row.password_hash))) return json(errorBody('invalid_credentials'), { status: 403 });
  return json({ ok: true });
}

// ---- installs linked to the account (one row per device and tool) ------------------------------
export async function recordInstall(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  if (!ctx.deviceId) throw new HttpError(400, 'device_required');
  const b = await body(ctx, 1_000);
  const tool = checkTool(b.tool);
  if (typeof b.version !== 'string' || !VERSION.test(b.version)) throw new HttpError(400, 'invalid_version');
  if (typeof b.target !== 'string' || !TARGET.test(b.target)) throw new HttpError(400, 'invalid_target');
  await ctx.env.DB
    .prepare('INSERT INTO installs (user_id, device_id, tool, version, target, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(device_id, tool) DO UPDATE SET version = ?4, target = ?5, updated_at = ?6')
    .bind(uid, ctx.deviceId, tool, b.version, b.target, Date.now())
    .run();
  return json({ ok: true });
}

export async function removeInstall(ctx: Ctx, p: Record<string, string>): Promise<Response> {
  await requireUser(ctx);
  if (!ctx.deviceId) throw new HttpError(400, 'device_required');
  const tool = checkTool(p.tool);
  await ctx.env.DB.prepare('DELETE FROM installs WHERE device_id = ?1 AND tool = ?2').bind(ctx.deviceId, tool).run();
  return json({ ok: true });
}

export async function listInstalls(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  const r = await ctx.env.DB
    .prepare('SELECT i.tool, i.version, i.target, i.updated_at, d.id AS device_id, d.name AS device_name FROM installs i JOIN devices d ON d.id = i.device_id WHERE i.user_id = ?1 ORDER BY d.name, i.tool')
    .bind(uid)
    .all();
  return json({ items: r.results });
}
