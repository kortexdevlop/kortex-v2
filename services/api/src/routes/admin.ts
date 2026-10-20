import type { Ctx } from '../env.ts';
import { aiStats } from '../credits.ts';
import { HttpError, allowedOrigins, json } from '../http.ts';
import { rateLimit } from '../ratelimit.ts';
import { requireSession } from '../session.ts';
import releases from '../releases.json' with { type: 'json' };

/**
 * The owner's dashboard data (`GET /v1/admin/overview`). Only a signed-in account whose e-mail is in
 * the `ADMIN_EMAILS` secret (comma-separated) gets it; everyone else, and every deployment where the
 * secret is not set, sees a plain 404 as if the route did not exist. Aggregates only: no content,
 * no raw account id, no e-mail.
 */
async function requireAdmin(ctx: Ctx): Promise<void> {
  const allowed = (ctx.env.ADMIN_EMAILS ?? '').split(',').map((e) => e.trim().toLowerCase()).filter(Boolean);
  if (allowed.length === 0) throw new HttpError(404, 'not_found');
  const uid = await requireSession(ctx);
  const row = await ctx.env.DB.prepare('SELECT email FROM users WHERE id = ?1').bind(uid).first<{ email: string }>();
  if (!row || !allowed.includes(row.email.toLowerCase())) throw new HttpError(404, 'not_found');
  await rateLimit(ctx, `admin:${uid}`, 30, 60);
}

interface HostStatus { host: string; ok: boolean; status: number; ms: number }

/** One cheap request per public host, from the server side, so the owner sees what visitors see. */
async function probe(host: string): Promise<HostStatus> {
  const started = Date.now();
  try {
    const res = await fetch(`https://${host}/`, { method: 'HEAD', redirect: 'manual', signal: AbortSignal.timeout(4000) });
    return { host, ok: res.status < 500, status: res.status, ms: Date.now() - started };
  } catch {
    return { host, ok: false, status: 0, ms: Date.now() - started };
  }
}

export async function overview(ctx: Ctx): Promise<Response> {
  await requireAdmin(ctx);
  const db = ctx.env.DB;
  const now = Date.now();
  const since = new Date(now - 6 * 86_400_000).toISOString().slice(0, 10);
  const hosts = [...new Set([...allowedOrigins(ctx.env).filter((o) => o.startsWith('https://')).map((o) => new URL(o).hostname), new URL(ctx.req.url).hostname])];
  const count = async (table: string): Promise<number> => (await db.prepare(`SELECT COUNT(*) AS n FROM ${table}`).first<{ n: number }>())?.n ?? 0;
  const [ai, errors, probes, users, sessions, syncItems, buckets, limiter] = await Promise.all([
    aiStats(ctx.env, now),
    db.prepare('SELECT day, app, kind, n FROM client_errors WHERE day >= ?1 ORDER BY day DESC, app, kind').bind(since).all<{ day: string; app: string; kind: string; n: number }>(),
    Promise.all(hosts.map(probe)),
    count('users'), count('sessions'), count('sync_items'), count('ai_buckets'), count('rate_limits'),
  ]);
  const meta = ctx.env.CF_VERSION_METADATA;
  return json({
    generated_at: now,
    api_version: meta ? { id: meta.id, deployed_at: meta.timestamp } : null,
    ai,
    errors: errors.results,
    hosts: probes,
    database: { users, sessions, sync_items: syncItems, ai_buckets: buckets, rate_limits: limiter },
    releases: releases.publications,
  });
}
