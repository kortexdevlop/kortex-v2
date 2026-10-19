import type { Ctx } from '../env.ts';
import { HttpError, isRecord, readJson } from '../http.ts';
import { rateLimit } from '../ratelimit.ts';

/**
 * Light error monitoring (MASTER_PROMPT phase 12): an app reports that it failed, nothing else.
 * The body is `{app, kind}` from two fixed lists; the server keeps a daily counter per pair. No
 * account, no IP address, no message and no page address is ever stored. Clients skip it when the
 * browser sends Do Not Track or Global Privacy Control, and send at most three reports per page.
 */
const APPS = new Set(['flow', 'agent', 'search', 'workspace', 'dashboard', 'store', 'run']);
const KINDS = new Set(['panic', 'unhandled']);

export async function report(ctx: Ctx): Promise<Response> {
  const b = await readJson(ctx.req, 300);
  if (!isRecord(b) || typeof b.app !== 'string' || typeof b.kind !== 'string' || !APPS.has(b.app) || !KINDS.has(b.kind)) {
    throw new HttpError(400, 'invalid_report');
  }
  // The address is only used for this limit (hashed, expiring), never stored with the report.
  await rateLimit(ctx, `telemetry:${ctx.ip}`, 30, 3600);
  const day = new Date().toISOString().slice(0, 10);
  await ctx.env.DB
    .prepare('INSERT INTO client_errors (day, app, kind, n) VALUES (?1, ?2, ?3, 1) ON CONFLICT(day, app, kind) DO UPDATE SET n = n + 1')
    .bind(day, b.app, b.kind)
    .run();
  return new Response(null, { status: 204 });
}
