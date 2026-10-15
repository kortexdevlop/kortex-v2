import type { Ctx, Env } from './env.ts';
import { HttpError, allowedOrigins, errorBody, json } from './http.ts';
import * as auth from './routes/auth.ts';
import * as data from './routes/data.ts';

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
          'Access-Control-Allow-Headers': 'Content-Type, X-Kortex-Request',
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

export default { fetch: handle } satisfies ExportedHandler<Env>;
