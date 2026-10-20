import type { Ctx } from './env.ts';
import { sha256Hex } from './crypto.ts';
import { HttpError } from './http.ts';

/** Fixed-window limiter backed by D1. `key` should already include the route and the client. */
export async function rateLimit(ctx: Ctx, key: string, limit: number, windowSeconds: number, now = Date.now()): Promise<void> {
  const windowStart = Math.floor(now / 1000 / windowSeconds) * windowSeconds;
  const row = await ctx.env.DB
    .prepare(
      `INSERT INTO rate_limits (key, window_start, count) VALUES (?1, ?2, 1)
       ON CONFLICT(key) DO UPDATE SET
         count = CASE WHEN window_start = ?2 THEN count + 1 ELSE 1 END,
         window_start = ?2
       RETURNING count`,
    )
    .bind(await sha256Hex(key), windowStart)
    .first<{ count: number }>();
  if ((row?.count ?? 1) > limit) throw new HttpError(429, 'rate_limited');
}
