import type { Ctx } from '../env.ts';
import { HttpError, allowedOrigins, isRecord, json, readJson } from '../http.ts';
import { rateLimit } from '../ratelimit.ts';
import { requireSession } from '../session.ts';

/**
 * OAuth broker, only for providers that refuse PKCE without a client secret. It holds the client
 * secrets (Worker secrets `OAUTH_<PROVIDER>_ID` / `_SECRET`), exchanges a code or a refresh token
 * and hands the tokens straight back to the signed-in user: nothing is stored or logged here,
 * tokens live on the user's device.
 */
interface Provider { token: string; basic?: boolean; json?: boolean }

const PROVIDERS: Record<string, Provider> = {
  github: { token: 'https://github.com/login/oauth/access_token' },
  google: { token: 'https://oauth2.googleapis.com/token' },
  notion: { token: 'https://api.notion.com/v1/oauth/token', basic: true, json: true },
  dropbox: { token: 'https://api.dropboxapi.com/oauth2/token' },
  slack: { token: 'https://slack.com/api/oauth.v2.access' },
  discord: { token: 'https://discord.com/api/oauth2/token' },
};

const secretOf = (ctx: Ctx, name: string): { id: string; secret: string } | null => {
  const key = name.toUpperCase();
  const id = ctx.env[`OAUTH_${key}_ID`];
  const secret = ctx.env[`OAUTH_${key}_SECRET`];
  return id && secret ? { id, secret } : null;
};

export async function providers(ctx: Ctx): Promise<Response> {
  await requireSession(ctx);
  return json({ providers: Object.keys(PROVIDERS).filter((p) => secretOf(ctx, p)) });
}

/** Web apps of ours, or a loopback address for the desktop flow (RFC 8252). */
export function redirectAllowed(ctx: Ctx, raw: unknown): raw is string {
  if (typeof raw !== 'string' || raw.length > 500) return false;
  let url: URL;
  try { url = new URL(raw); } catch { return false; }
  if (url.username || url.password || url.hash) return false;
  if (url.protocol === 'http:') return url.hostname === '127.0.0.1' || url.hostname === '[::1]';
  return url.protocol === 'https:' && allowedOrigins(ctx.env).includes(url.origin);
}

async function tokenRequest(ctx: Ctx, grant: 'authorization_code' | 'refresh_token'): Promise<Response> {
  const userId = await requireSession(ctx);
  const body = await readJson(ctx.req, 4_000);
  if (!isRecord(body) || typeof body.provider !== 'string' || !Object.hasOwn(PROVIDERS, body.provider)) throw new HttpError(400, 'unknown_provider');
  const provider = PROVIDERS[body.provider]!;
  const creds = secretOf(ctx, body.provider);
  if (!creds) throw new HttpError(503, 'provider_unavailable');
  await rateLimit(ctx, `oauth:${userId}`, 30, 3600);
  const form: Record<string, string> = { grant_type: grant };
  if (grant === 'authorization_code') {
    if (typeof body.code !== 'string' || body.code.length < 1 || body.code.length > 2_000) throw new HttpError(400, 'invalid_code');
    if (!redirectAllowed(ctx, body.redirect_uri)) throw new HttpError(400, 'redirect_not_allowed');
    form.code = body.code;
    form.redirect_uri = body.redirect_uri;
    if (typeof body.code_verifier === 'string' && /^[A-Za-z0-9._~-]{43,128}$/.test(body.code_verifier)) form.code_verifier = body.code_verifier;
  } else {
    if (typeof body.refresh_token !== 'string' || body.refresh_token.length < 1 || body.refresh_token.length > 4_000) throw new HttpError(400, 'invalid_token');
    form.refresh_token = body.refresh_token;
  }
  const headers: Record<string, string> = { Accept: 'application/json' };
  let payload: string;
  if (provider.basic) headers.Authorization = `Basic ${btoa(`${creds.id}:${creds.secret}`)}`;
  else { form.client_id = creds.id; form.client_secret = creds.secret; }
  if (provider.json) { headers['Content-Type'] = 'application/json'; payload = JSON.stringify(form); }
  else { headers['Content-Type'] = 'application/x-www-form-urlencoded'; payload = new URLSearchParams(form).toString(); }
  let res: Response;
  try { res = await fetch(provider.token, { method: 'POST', headers, body: payload, redirect: 'manual', signal: AbortSignal.timeout(10_000) }); }
  catch { throw new HttpError(502, 'oauth_unreachable'); }
  let data: unknown = null;
  try { data = await res.json(); } catch { /* not JSON */ }
  if (!res.ok || !isRecord(data) || typeof data.access_token !== 'string') {
    const reason = isRecord(data) && typeof data.error === 'string' && /^[a-z_]{1,40}$/.test(data.error) ? data.error : 'failed';
    return json({ error: 'oauth_failed', provider_error: reason }, { status: 502 });
  }
  // Only the fields a client needs; nothing else from the provider is relayed.
  const pick = (k: string): unknown => (typeof data[k] === 'string' || typeof data[k] === 'number' ? data[k] : undefined);
  return json({ access_token: data.access_token, refresh_token: pick('refresh_token'), expires_in: pick('expires_in'), token_type: pick('token_type'), scope: pick('scope') });
}

export const exchange = (ctx: Ctx): Promise<Response> => tokenRequest(ctx, 'authorization_code');
export const refresh = (ctx: Ctx): Promise<Response> => tokenRequest(ctx, 'refresh_token');
