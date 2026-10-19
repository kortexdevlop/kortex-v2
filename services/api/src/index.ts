import type { Ctx, Env } from './env.ts';
import { HttpError, allowedOrigins, errorBody, json } from './http.ts';
import * as auth from './routes/auth.ts';
import * as security from './routes/security.ts';
import * as passkeys from './routes/passkeys.ts';
import * as ai from './routes/ai.ts';
import * as data from './routes/data.ts';
import * as dev from './routes/devices.ts';
import * as proxy from './routes/proxy.ts';
import * as oauth from './routes/oauth.ts';
import * as telemetry from './routes/telemetry.ts';

type Handler = (ctx: Ctx, params: Record<string, string>) => Promise<Response>;
const routes: [string, RegExp, Handler][] = [];
const add = (method: string, path: string, h: Handler): void => {
  const re = new RegExp('^' + path.replace(/:([a-z]+)/g, '(?<$1>[^/]+)') + '$');
  routes.push([method, re, h]);
};

add('GET', '/v1/health', async () => json({ ok: true }));
add('POST', '/v1/auth/register', auth.register);
add('POST', '/v1/auth/login', auth.login);
add('POST', '/v1/auth/logout', auth.logout);
add('GET', '/v1/me/sessions', security.listSessions);
add('POST', '/v1/me/sessions/revoke', security.revokeSessions);
add('POST', '/v1/me/password', security.changePassword);
add('GET', '/v1/me/passkeys', passkeys.list);
add('DELETE', '/v1/me/passkeys/:id', passkeys.remove);
add('POST', '/v1/me/passkeys/options', passkeys.registrationOptions);
add('POST', '/v1/me/passkeys/verify', passkeys.registrationVerify);
add('POST', '/v1/auth/passkey/options', passkeys.authenticationOptions);
add('POST', '/v1/auth/passkey/verify', passkeys.authenticationVerify);
add('POST', '/v1/auth/device', dev.createDevice);
add('POST', '/v1/auth/confirm', dev.confirm);
add('GET', '/v1/devices', dev.listDevices);
add('DELETE', '/v1/devices/:id', dev.deleteDevice);
add('POST', '/v1/installs', dev.recordInstall);
add('GET', '/v1/installs', dev.listInstalls);
add('DELETE', '/v1/installs/:tool', dev.removeInstall);
add('GET', '/v1/me', auth.me);
add('DELETE', '/v1/me', auth.deleteMe);
add('GET', '/v1/me/export', auth.exportMe);
add('GET', '/v1/settings', data.getSettings);
add('PUT', '/v1/settings', data.putSettings);
add('GET', '/v1/sync', data.listSync);
add('GET', '/v1/sync/:key', data.getSync);
add('PUT', '/v1/sync/:key', data.putSync);
add('DELETE', '/v1/sync/:key', data.deleteSync);
add('POST', '/v1/usage', data.postUsage);
add('GET', '/v1/usage', data.getUsage);
add('GET', '/v1/ai/credits', ai.getCredits);
add('POST', '/v1/ai/chat', ai.chat);
add('POST', '/v1/proxy', proxy.forward);
add('GET', '/v1/oauth/providers', oauth.providers);
add('POST', '/v1/oauth/exchange', oauth.exchange);
add('POST', '/v1/oauth/refresh', oauth.refresh);
add('POST', '/v1/telemetry', telemetry.report);

const SECURITY_HEADERS = {
  'X-Content-Type-Options': 'nosniff',
  'Referrer-Policy': 'no-referrer',
  'Cross-Origin-Resource-Policy': 'same-site',
  'Strict-Transport-Security': 'max-age=63072000; includeSubDomains; preload',
};

function withCors(res: Response, origin: string | null, env: Env): Response {
  const out = new Response(res.body, res);
  for (const [k, v] of Object.entries(SECURITY_HEADERS)) out.headers.set(k, v);
  out.headers.append('Vary', 'Origin');
  if (origin && allowedOrigins(env).includes(origin)) {
    out.headers.set('Access-Control-Allow-Origin', origin);
    out.headers.set('Access-Control-Allow-Credentials', 'true');
  }
  return out;
}

/** CSRF: browsers always send `Origin` on cross-site writes. Present => must be allow-listed.
 *  Absent (CLI, server-to-server) => must carry the custom header, which a cross-site form cannot set. */
function checkCsrf(req: Request, env: Env): void {
  if (req.method === 'GET' || req.method === 'HEAD') return;
  const origin = req.headers.get('origin');
  if (origin) {
    if (!allowedOrigins(env).includes(origin)) throw new HttpError(403, 'forbidden_origin');
  } else if (req.headers.get('x-kortex-request') !== '1') {
    throw new HttpError(403, 'missing_csrf_header');
  }
}

export async function handle(req: Request, env: Env): Promise<Response> {
  const origin = req.headers.get('origin');
  const url = new URL(req.url);
  if (req.method === 'OPTIONS') {
    if (!origin || !allowedOrigins(env).includes(origin)) return new Response(null, { status: 403 });
    return withCors(
      new Response(null, {
        status: 204,
        headers: {
          'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE',
          'Access-Control-Allow-Headers': 'Content-Type, X-Kortex-Request, X-Kortex-User, Authorization',
          'Access-Control-Max-Age': '600',
        },
      }),
      origin,
      env,
    );
  }
  const ctx: Ctx = { env, req, url, ip: req.headers.get('cf-connecting-ip') ?? 'local' };
  try {
    checkCsrf(req, env);
    for (const [method, re, handler] of routes) {
      if (method !== req.method) continue;
      const m = re.exec(url.pathname);
      if (m) return withCors(await handler(ctx, { ...m.groups }), origin, env);
    }
    return withCors(json(errorBody('not_found'), { status: 404 }), origin, env);
  } catch (e) {
    if (e instanceof HttpError) return withCors(json(errorBody(e.code), { status: e.status }), origin, env);
    console.error('unhandled', e instanceof Error ? e.message : 'unknown');
    return withCors(json(errorBody('internal'), { status: 500 }), origin, env);
  }
}

/** Remove expired security metadata without retaining raw IP addresses or login emails. */
export async function cleanup(env: Env, now = Date.now()): Promise<void> {
  await env.DB.prepare('DELETE FROM webauthn_challenges WHERE expires_at <= ?1').bind(now).run();
  const cutoff = new Date(now - 90 * 86_400_000).toISOString().slice(0, 10);
  await env.DB.prepare('DELETE FROM sessions WHERE expires_at <= ?1').bind(now).run();
  await env.DB.prepare('DELETE FROM rate_limits WHERE window_start < ?1').bind(Math.floor(now / 1000) - 86_400).run();
  for (const table of ['usage_daily', 'ai_ledger', 'ai_global', 'client_errors', 'ai_user_day', 'ai_app']) {
    await env.DB.prepare(`DELETE FROM ${table} WHERE day < ?1`).bind(cutoff).run();
  }
}

export default { fetch: handle, scheduled: async (_event, env) => { await cleanup(env); } } satisfies ExportedHandler<Env>;
