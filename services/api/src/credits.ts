import type { Env } from './env.ts';

/** Defaults: generous per user, hard-capped globally under the provider's free quota. */
export interface Limits { userCap: number; refillPer3h: number; globalDaily: number }

export const limits = (env: Env): Limits => ({
  userCap: Number(env.AI_USER_CAP ?? '1200'),
  refillPer3h: Number(env.AI_USER_REFILL_3H ?? '400'),
  globalDaily: Number(env.AI_GLOBAL_DAILY ?? '9000'), // 10 000 free neurons/day, 10 % safety margin
});

const THREE_HOURS_MS = 3 * 3600 * 1000;
export const utcDay = (now: number): string => new Date(now).toISOString().slice(0, 10);
export const secondsToUtcMidnight = (now: number): number => {
  const d = new Date(now);
  return Math.ceil((Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() + 1) - now) / 1000);
};

interface Bucket { tokens: number; updated_at: number }

async function readBucket(env: Env, uid: string, now: number): Promise<Bucket> {
  const l = limits(env);
  const row = await env.DB.prepare('SELECT tokens, updated_at FROM ai_buckets WHERE user_id = ?1').bind(uid).first<Bucket>();
  if (!row) return { tokens: l.userCap, updated_at: now };
  const refilled = row.tokens + ((now - row.updated_at) / THREE_HOURS_MS) * l.refillPer3h;
  return { tokens: Math.min(l.userCap, refilled), updated_at: row.updated_at };
}

export interface CreditsView {
  percent: number;
  status: 'ok' | 'low' | 'empty' | 'paused';
  /** Seconds until something changes for the user (next refill step or global reset). */
  resets_in_seconds: number | null;
}

/** What the UI shows: a percentage only, never raw neuron counts. */
export async function creditsView(env: Env, uid: string, now = Date.now()): Promise<CreditsView> {
  const l = limits(env);
  const b = await readBucket(env, uid, now);
  const percent = Math.max(0, Math.min(100, Math.round((b.tokens / l.userCap) * 100)));
  const g = await env.DB.prepare('SELECT used FROM ai_global WHERE day = ?1').bind(utcDay(now)).first<{ used: number }>();
  if ((g?.used ?? 0) >= l.globalDaily) return { percent, status: 'paused', resets_in_seconds: secondsToUtcMidnight(now) };
  if (percent === 0) return { percent, status: 'empty', resets_in_seconds: Math.ceil(((1 - (b.tokens % 1)) / l.refillPer3h) * (THREE_HOURS_MS / 1000)) };
  return { percent, status: percent <= 15 ? 'low' : 'ok', resets_in_seconds: null };
}

export type Reservation = { ok: true; cost: number } | { ok: false; reason: 'user_empty' | 'ai_paused'; retryAfter: number };

/** Reserve `cost` neurons from the user's bucket AND the global day budget, atomically enough:
 *  the global counter uses a conditional upsert, the bucket an optimistic update with retries. */
export async function reserve(env: Env, uid: string, cost: number, now = Date.now()): Promise<Reservation> {
  const l = limits(env);
  const day = utcDay(now);
  for (let attempt = 0; attempt < 4; attempt++) {
    const b = await readBucket(env, uid, now);
    if (b.tokens < cost) {
      const missing = cost - b.tokens;
      return { ok: false, reason: 'user_empty', retryAfter: Math.ceil((missing / l.refillPer3h) * (THREE_HOURS_MS / 1000)) };
    }
    const g = await env.DB
      .prepare(// The first request of the day must respect the cap too: guard the INSERT itself, not only the update.
      'INSERT INTO ai_global (day, used) SELECT ?1, ?2 WHERE ?2 <= ?3 ON CONFLICT(day) DO UPDATE SET used = used + ?2 WHERE used + ?2 <= ?3 RETURNING used')
      .bind(day, cost, l.globalDaily)
      .first<{ used: number }>();
    if (!g) return { ok: false, reason: 'ai_paused', retryAfter: secondsToUtcMidnight(now) };
    const left = b.tokens - cost;
    const exists = await env.DB.prepare('SELECT updated_at FROM ai_buckets WHERE user_id = ?1').bind(uid).first<{ updated_at: number }>();
    let changes: number;
    if (!exists) {
      const r = await env.DB.prepare('INSERT INTO ai_buckets (user_id, tokens, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(user_id) DO NOTHING').bind(uid, left, now).run();
      changes = r.meta.changes;
    } else {
      const r = await env.DB.prepare('UPDATE ai_buckets SET tokens = ?2, updated_at = ?3 WHERE user_id = ?1 AND updated_at = ?4').bind(uid, left, now, exists.updated_at).run();
      changes = r.meta.changes;
    }
    if (changes === 1) return { ok: true, cost };
    // Lost a race with a concurrent request: give the global budget back and retry.
    await env.DB.prepare('UPDATE ai_global SET used = MAX(0, used - ?2) WHERE day = ?1').bind(day, cost).run();
  }
  return { ok: false, reason: 'user_empty', retryAfter: 1 };
}

/** After the call: refund the unused part of the reservation (or charge the overrun). */
export async function settle(env: Env, uid: string, reserved: number, actual: number, model: string, now = Date.now()): Promise<void> {
  const l = limits(env);
  const diff = reserved - actual; // > 0: refund
  const day = utcDay(now);
  await env.DB.prepare('UPDATE ai_global SET used = MAX(0, used - ?2) WHERE day = ?1').bind(day, diff).run();
  await env.DB.prepare('UPDATE ai_buckets SET tokens = MAX(0, MIN(?3, tokens + ?2)) WHERE user_id = ?1').bind(uid, diff, l.userCap).run();
  await env.DB
    .prepare('INSERT INTO ai_ledger (day, model, requests, neurons) VALUES (?1, ?2, 1, ?3) ON CONFLICT(day, model) DO UPDATE SET requests = requests + 1, neurons = neurons + ?3')
    .bind(day, model, actual)
    .run();
}

/** The provider said "daily limit reached": stop spending for today whatever our own counter says. */
export async function markGloballyExhausted(env: Env, now = Date.now()): Promise<void> {
  await env.DB
    .prepare('INSERT INTO ai_global (day, used) VALUES (?1, ?2) ON CONFLICT(day) DO UPDATE SET used = MAX(used, ?2)')
    .bind(utcDay(now), limits(env).globalDaily)
    .run();
}
