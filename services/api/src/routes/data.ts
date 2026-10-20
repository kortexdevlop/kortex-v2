import type { Ctx } from '../env.ts';
import { HttpError, errorBody, isRecord, json, readJson } from '../http.ts';
import { requireUser } from '../session.ts';
import { MAX_BLOB_BYTES, checkBlob, checkKey, checkTool } from '../validate.ts';

const MAX_KEYS = 200;
const MAX_SETTINGS_BYTES = 16 * 1024;

/** Bind each backup request to the account the client displayed, even across cookie rotation. */
async function requireSyncUser(ctx: Ctx): Promise<string> {
  const uid = await requireUser(ctx);
  const expected = ctx.req.headers.get('x-kortex-user');
  if (expected !== null && expected !== uid) throw new HttpError(409, 'session_changed');
  return uid;
}

// ---- settings: one JSON blob per user --------------------------------------------------------
export async function getSettings(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  const row = await ctx.env.DB.prepare('SELECT data, updated_at FROM settings WHERE user_id = ?1').bind(uid).first<{ data: string; updated_at: number }>();
  return json(row ? { data: JSON.parse(row.data), updated_at: row.updated_at } : { data: {}, updated_at: 0 });
}

export async function putSettings(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  const b = await readJson(ctx.req, MAX_SETTINGS_BYTES + 100);
  if (!isRecord(b) || !isRecord(b.data)) throw new HttpError(400, 'invalid_data');
  const text = JSON.stringify(b.data);
  if (new TextEncoder().encode(text).length > MAX_SETTINGS_BYTES) throw new HttpError(413, 'blob_too_large');
  const now = Date.now();
  await ctx.env.DB
    .prepare('INSERT INTO settings (user_id, data, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(user_id) DO UPDATE SET data = ?2, updated_at = ?3')
    .bind(uid, text, now)
    .run();
  return json({ ok: true, updated_at: now });
}

// ---- sync: small versioned blobs with optimistic concurrency ---------------------------------
export async function listSync(ctx: Ctx): Promise<Response> {
  const uid = await requireSyncUser(ctx);
  const r = await ctx.env.DB.prepare('SELECT key, version, updated_at FROM sync_items WHERE user_id = ?1 ORDER BY key').bind(uid).all();
  return json({ items: r.results });
}

export async function getSync(ctx: Ctx, p: Record<string, string>): Promise<Response> {
  const uid = await requireSyncUser(ctx);
  const key = checkKey(p.key ?? '');
  const row = await ctx.env.DB.prepare('SELECT version, data, updated_at FROM sync_items WHERE user_id = ?1 AND key = ?2').bind(uid, key).first<{ version: number; data: string; updated_at: number }>();
  if (!row) return json(errorBody('not_found'), { status: 404 });
  return json({ key, version: row.version, data: JSON.parse(row.data), updated_at: row.updated_at });
}

/** Body `{ data, version }`: `version` is the version the client last saw (0 to create). */
export async function putSync(ctx: Ctx, p: Record<string, string>): Promise<Response> {
  const uid = await requireSyncUser(ctx);
  const key = checkKey(p.key ?? '');
  const b = await readJson(ctx.req, MAX_BLOB_BYTES + 200);
  if (!isRecord(b) || !('data' in b) || !Number.isInteger(b.version) || (b.version as number) < 0) throw new HttpError(400, 'invalid_data');
  const text = checkBlob(b.data);
  const expected = b.version as number;
  const now = Date.now();
  const db = ctx.env.DB;

  if (expected === 0) {
    const count = await db.prepare('SELECT COUNT(*) AS n FROM sync_items WHERE user_id = ?1').bind(uid).first<{ n: number }>();
    if ((count?.n ?? 0) >= MAX_KEYS) throw new HttpError(409, 'too_many_keys');
    const res = await db
      .prepare('INSERT INTO sync_items (user_id, key, version, data, updated_at) VALUES (?1, ?2, 1, ?3, ?4) ON CONFLICT(user_id, key) DO NOTHING')
      .bind(uid, key, text, now)
      .run();
    if (res.meta.changes === 1) return json({ key, version: 1, updated_at: now }, { status: 201 });
  } else {
    const res = await db
      .prepare('UPDATE sync_items SET data = ?3, version = version + 1, updated_at = ?4 WHERE user_id = ?1 AND key = ?2 AND version = ?5')
      .bind(uid, key, text, now, expected)
      .run();
    if (res.meta.changes === 1) return json({ key, version: expected + 1, updated_at: now });
  }
  // Someone else wrote first: return the current copy so the client can merge field by field.
  const cur = await db.prepare('SELECT version, data, updated_at FROM sync_items WHERE user_id = ?1 AND key = ?2').bind(uid, key).first<{ version: number; data: string; updated_at: number }>();
  return json({ error: 'version_conflict', current: cur ? { version: cur.version, data: JSON.parse(cur.data), updated_at: cur.updated_at } : null }, { status: 409 });
}

export async function deleteSync(ctx: Ctx, p: Record<string, string>): Promise<Response> {
  const uid = await requireSyncUser(ctx);
  const key = checkKey(p.key ?? '');
  await ctx.env.DB.prepare('DELETE FROM sync_items WHERE user_id = ?1 AND key = ?2').bind(uid, key).run();
  return json({ ok: true });
}

// ---- usage: aggregated daily counters, kept 90 days ------------------------------------------
const today = (now = Date.now()): string => new Date(now).toISOString().slice(0, 10);

export async function postUsage(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  const b = await readJson(ctx.req, 1_000);
  if (!isRecord(b)) throw new HttpError(400, 'invalid_json');
  const tool = checkTool(b.tool);
  const n = b.count === undefined ? 1 : b.count;
  if (!Number.isInteger(n) || (n as number) < 1 || (n as number) > 1000) throw new HttpError(400, 'invalid_count');
  const now = Date.now();
  const db = ctx.env.DB;
  await db
    .prepare('INSERT INTO usage_daily (user_id, day, tool, count) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(user_id, day, tool) DO UPDATE SET count = count + ?4')
    .bind(uid, today(now), tool, n)
    .run();
  const cutoff = today(now - 90 * 86_400_000);
  await db.prepare('DELETE FROM usage_daily WHERE user_id = ?1 AND day < ?2').bind(uid, cutoff).run();
  return json({ ok: true });
}

export async function getUsage(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  const days = Math.min(90, Math.max(1, Number(ctx.url.searchParams.get('days') ?? '30') || 30));
  const since = today(Date.now() - (days - 1) * 86_400_000);
  const r = await ctx.env.DB.prepare('SELECT day, tool, count FROM usage_daily WHERE user_id = ?1 AND day >= ?2 ORDER BY day, tool').bind(uid, since).all();
  return json({ days, items: r.results });
}
