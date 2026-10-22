import type { Env } from './env.ts';
import { sha256Hex } from './crypto.ts';

/** Defaults: generous per user, hard-capped globally under the provider's free quota. */
export interface Limits { userCap: number; refillPer3h: number; globalDaily: number; userDaily: number }

export const limits = (env: Env): Limits => ({
  userCap: Number(env.AI_USER_CAP ?? '1200'),
  refillPer3h: Number(env.AI_USER_REFILL_3H ?? '400'),
  globalDaily: Number(env.AI_GLOBAL_DAILY ?? '9000'), // 10 000 free neurons/day, 10 % safety margin
  userDaily: Number(env.AI_USER_DAILY ?? '2500'), // no single account may take more than ~28 % of the day
});

/** Refill slows down as the shared daily budget runs out, so the last hours stay fair for everyone. */
export const refillFactor = (used: number, globalDaily: number): number =>
  used >= globalDaily * 0.9 ? 0.25 : used >= globalDaily * 0.7 ? 0.5 : 1;

async function globalUsed(env: Env, now: number): Promise<number> {
  const g = await env.DB.prepare('SELECT used FROM ai_global WHERE day = ?1').bind(utcDay(now)).first<{ used: number }>();
  return g?.used ?? 0;
}

async function userSpentToday(env: Env, uid: string, now: number): Promise<number> {
  const r = await env.DB.prepare('SELECT neurons FROM ai_user_day WHERE day = ?1 AND user_id = ?2').bind(utcDay(now), uid).first<{ neurons: number }>();
  return r?.neurons ?? 0;
}

const THREE_HOURS_MS = 3 * 3600 * 1000;
export const utcDay = (now: number): string => new Date(now).toISOString().slice(0, 10);
export const secondsToUtcMidnight = (now: number): number => {
  const d = new Date(now);
  return Math.ceil((Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() + 1) - now) / 1000);
};

interface Bucket { tokens: number; updated_at: number; storedTokens?: number }

async function readBucket(env: Env, uid: string, now: number, factor = 1): Promise<Bucket> {
  const l = limits(env);
  const row = await env.DB.prepare('SELECT tokens, updated_at FROM ai_buckets WHERE user_id = ?1').bind(uid).first<Bucket>();
  if (!row) return { tokens: l.userCap, updated_at: now };
  const refilled = row.tokens + ((now - row.updated_at) / THREE_HOURS_MS) * l.refillPer3h * factor;
  return { tokens: Math.min(l.userCap, refilled), updated_at: row.updated_at, storedTokens: row.tokens };
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
  const used = await globalUsed(env, now);
  const factor = refillFactor(used, l.globalDaily);
  const b = await readBucket(env, uid, now, factor);
  const percent = Math.max(0, Math.min(100, Math.round((b.tokens / l.userCap) * 100)));
  if (used >= l.globalDaily) return { percent, status: 'paused', resets_in_seconds: secondsToUtcMidnight(now) };
  // The account's own share of the day is spent: nothing comes back before the next UTC day.
  if ((await userSpentToday(env, uid, now)) >= l.userDaily - 1) return { percent: Math.min(percent, 1), status: 'empty', resets_in_seconds: secondsToUtcMidnight(now) };
  if (percent === 0) return { percent, status: 'empty', resets_in_seconds: Math.ceil(((1 - (b.tokens % 1)) / (l.refillPer3h * factor)) * (THREE_HOURS_MS / 1000)) };
  return { percent, status: percent <= 15 ? 'low' : 'ok', resets_in_seconds: null };
}

export type Reservation = { ok: true; cost: number; day: string } | { ok: false; reason: 'user_empty' | 'ai_paused' | 'user_daily'; retryAfter: number };

/** Reserve `cost` neurons from the user's bucket AND the global day budget, atomically enough:
 *  the global counter uses a conditional upsert, the bucket an optimistic update with retries. */
export async function reserve(env: Env, uid: string, cost: number, now = Date.now()): Promise<Reservation> {
  if (!Number.isFinite(cost) || cost < 0) throw new Error('invalid_reservation_cost');
  const l = limits(env);
  const day = utcDay(now);
  const factor = refillFactor(await globalUsed(env, now), l.globalDaily);
  for (let attempt = 0; attempt < 4; attempt++) {
    const b = await readBucket(env, uid, now, factor);
    if (b.tokens < cost) {
      const missing = cost - b.tokens;
      return { ok: false, reason: 'user_empty', retryAfter: Math.ceil((missing / (l.refillPer3h * factor)) * (THREE_HOURS_MS / 1000)) };
    }
    // The account's share of today first: a single heavy user can never drain the shared budget.
    const mine = await env.DB
      .prepare('INSERT INTO ai_user_day (day, user_id, neurons) SELECT ?1, ?2, ?3 WHERE ?3 <= ?4 ON CONFLICT(day, user_id) DO UPDATE SET neurons = neurons + ?3 WHERE neurons + ?3 <= ?4 RETURNING neurons')
      .bind(day, uid, cost, l.userDaily)
      .first<{ neurons: number }>();
    if (!mine) return { ok: false, reason: 'user_daily', retryAfter: secondsToUtcMidnight(now) };
    const refundMine = async (): Promise<void> => {
      await env.DB.prepare('UPDATE ai_user_day SET neurons = MAX(0, neurons - ?3) WHERE day = ?1 AND user_id = ?2').bind(day, uid, cost).run();
    };
    const g = await env.DB
      .prepare(// The first request of the day must respect the cap too: guard the INSERT itself, not only the update.
      'INSERT INTO ai_global (day, used) SELECT ?1, ?2 WHERE ?2 <= ?3 ON CONFLICT(day) DO UPDATE SET used = used + ?2 WHERE used + ?2 <= ?3 RETURNING used')
      .bind(day, cost, l.globalDaily)
      .first<{ used: number }>();
    if (!g) {
      await refundMine();
      return { ok: false, reason: 'ai_paused', retryAfter: secondsToUtcMidnight(now) };
    }
    const left = b.tokens - cost;
    let changes: number;
    if (b.storedTokens === undefined) {
      const r = await env.DB.prepare('INSERT INTO ai_buckets (user_id, tokens, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(user_id) DO NOTHING').bind(uid, left, now).run();
      changes = r.meta.changes;
    } else {
      // Compare the exact snapshot used to compute `left`, including tokens: settlements can
      // change the balance without moving updated_at, and requests can share a millisecond.
      const r = await env.DB.prepare('UPDATE ai_buckets SET tokens = ?2, updated_at = ?3 WHERE user_id = ?1 AND updated_at = ?4 AND tokens = ?5').bind(uid, left, now, b.updated_at, b.storedTokens).run();
      changes = r.meta.changes;
    }
    if (changes === 1) return { ok: true, cost, day };
    // Lost a race with a concurrent request: give the global budget back and retry.
    await env.DB.prepare('UPDATE ai_global SET used = MAX(0, used - ?2) WHERE day = ?1').bind(day, cost).run();
    await refundMine();
  }
  return { ok: false, reason: 'user_empty', retryAfter: 1 };
}

/** After the call: refund the unused part of the reservation (or charge the overrun). */
export async function settle(env: Env, uid: string, reserved: number, actual: number, model: string, now = Date.now(), app = 'other', reservationDay = utcDay(now)): Promise<void> {
  if (![reserved, actual].every((n) => Number.isFinite(n) && n >= 0)) throw new Error('invalid_settlement_cost');
  const l = limits(env);
  const diff = reserved - actual; // > 0: refund
  // Calls may finish after UTC midnight; always settle against their reservation's day.
  const day = reservationDay;
  await env.DB.prepare('UPDATE ai_global SET used = MAX(0, used - ?2) WHERE day = ?1').bind(day, diff).run();
  await env.DB.prepare('UPDATE ai_buckets SET tokens = MAX(0, MIN(?3, tokens + ?2)) WHERE user_id = ?1').bind(uid, diff, l.userCap).run();
  await env.DB
    .prepare('UPDATE ai_user_day SET neurons = MAX(0, neurons - ?3), requests = requests + ?4 WHERE day = ?1 AND user_id = ?2')
    .bind(day, uid, diff, actual > 0 ? 1 : 0)
    .run();
  if (actual > 0) {
    await env.DB
      .prepare('INSERT INTO ai_app (day, app, requests, neurons) VALUES (?1, ?2, 1, ?3) ON CONFLICT(day, app) DO UPDATE SET requests = requests + 1, neurons = neurons + ?3')
      .bind(day, app, actual)
      .run();
  }
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

/** Which app asked, from the request's origin (`flow.kortex.site` is Flow, `agent.` is Mind, no
 *  origin is a desktop app or the CLI). Used for the owner's statistics only. */
export function appOf(req: Request): string {
  const origin = req.headers.get('origin');
  if (!origin) return 'desktop';
  const host = (() => { try { return new URL(origin).hostname; } catch { return ''; } })();
  const first = host.split('.')[0] ?? '';
  return ({ flow: 'flow', agent: 'mind', search: 'seek', workspace: 'desk', dashboard: 'hub', store: 'store', run: 'tools' } as Record<string, string>)[first] ?? 'site';
}

export interface AiStats {
  day: string;
  global: { used: number; cap: number; percent: number; factor: number };
  today: { requests: number; users: number; top: { user: string; neurons: number; requests: number }[] };
  apps: { app: string; requests: number; neurons: number }[];
  days: { day: string; requests: number; neurons: number }[];
  low_credit_users: number;
}

/** Owner statistics: never a raw user id (a short hash only) and never any content. */
export async function aiStats(env: Env, now = Date.now()): Promise<AiStats> {
  const l = limits(env);
  const day = utcDay(now);
  const used = await globalUsed(env, now);
  const since = utcDay(now - 6 * 86_400_000);
  const rows = (await env.DB.prepare('SELECT user_id, requests, neurons FROM ai_user_day WHERE day = ?1 ORDER BY neurons DESC LIMIT 5').bind(day).all<{ user_id: string; requests: number; neurons: number }>()).results;
  const totals = await env.DB.prepare('SELECT COUNT(*) AS users, COALESCE(SUM(requests), 0) AS requests FROM ai_user_day WHERE day = ?1').bind(day).first<{ users: number; requests: number }>();
  const apps = (await env.DB.prepare('SELECT app, SUM(requests) AS requests, SUM(neurons) AS neurons FROM ai_app WHERE day >= ?1 GROUP BY app ORDER BY neurons DESC').bind(since).all<{ app: string; requests: number; neurons: number }>()).results;
  const days = (await env.DB.prepare('SELECT day, SUM(requests) AS requests, SUM(neurons) AS neurons FROM ai_ledger WHERE day >= ?1 GROUP BY day ORDER BY day').bind(since).all<{ day: string; requests: number; neurons: number }>()).results;
  const low = await env.DB.prepare('SELECT COUNT(*) AS n FROM ai_buckets WHERE tokens < ?1').bind(l.userCap * 0.2).first<{ n: number }>();
  const short = async (id: string): Promise<string> => (await sha256Hex(`ai-stats:${id}`)).slice(0, 8);
  return {
    day,
    global: { used: Math.round(used), cap: l.globalDaily, percent: Math.round((used / l.globalDaily) * 100), factor: refillFactor(used, l.globalDaily) },
    today: {
      requests: totals?.requests ?? 0,
      users: totals?.users ?? 0,
      top: await Promise.all(rows.map(async (r) => ({ user: await short(r.user_id), neurons: Math.round(r.neurons), requests: r.requests }))),
    },
    apps: apps.map((a) => ({ ...a, neurons: Math.round(a.neurons) })),
    days: days.map((d) => ({ ...d, neurons: Math.round(d.neurons) })),
    low_credit_users: low?.n ?? 0,
  };
}
