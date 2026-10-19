import type { Ctx, Env } from '../env.ts';
import { HttpError, isRecord, json, readJson } from '../http.ts';
import { rateLimit } from '../ratelimit.ts';
import { requireSession } from '../session.ts';

/**
 * Network proxy for Flow in the browser, where CORS blocks most APIs. Desktop never uses it
 * (native requests). Signed-in users only, rate-limited, and bounded in every direction:
 *  - connector mode: an allow-listed API host may be called with any method and the caller's own
 *    credential header (their token, never ours);
 *  - free mode: any other public host, GET/HEAD only, no credentials, smaller limits.
 * Never reaches private networks, our own origins or non-HTTPS ports, and re-checks every redirect.
 */

const DEFAULT_CONNECTORS = [
  'api.github.com', 'googleapis.com', 'api.telegram.org', 'discord.com', 'slack.com',
  'api.notion.com', 'api.dropboxapi.com', 'content.dropboxapi.com', 'graph.microsoft.com',
];
const FREE_HEADERS = new Set(['accept', 'accept-language']);
const CONNECTOR_HEADERS = new Set([...FREE_HEADERS, 'content-type', 'authorization', 'x-api-key', 'notion-version']);
const RESPONSE_HEADERS = ['content-type', 'etag', 'last-modified', 'link', 'retry-after', 'content-language'];
const FREE_METHODS = new Set(['GET', 'HEAD']);
const CONNECTOR_METHODS = new Set(['GET', 'HEAD', 'POST', 'PUT', 'PATCH', 'DELETE']);
const MAX_REQUEST_BODY = 200_000;
const FREE_RESPONSE_BYTES = 1_000_000;
const CONNECTOR_RESPONSE_BYTES = 2_000_000;
const TIMEOUT_MS = 10_000;
const MAX_REDIRECTS = 3;
const BLOCKED_SUFFIXES = [
  '.local', '.localhost', '.internal', '.intranet', '.lan', '.home', '.corp', '.test',
  '.invalid', '.example', '.onion', '.arpa', '.kortex.site',
];

export const connectorHosts = (env: Env): string[] =>
  (env.PROXY_CONNECTORS ?? DEFAULT_CONNECTORS.join(',')).split(',').map((h) => h.trim().toLowerCase()).filter(Boolean);

export interface Target { url: URL; connector: boolean }

/** Validates a destination; throws 400/403 with a code the client can act on. */
export function checkTarget(raw: unknown, env: Env): Target {
  if (typeof raw !== 'string' || raw.length > 2_000) throw new HttpError(400, 'invalid_url');
  let url: URL;
  try { url = new URL(raw); } catch { throw new HttpError(400, 'invalid_url'); }
  if (url.protocol !== 'https:') throw new HttpError(400, 'https_only');
  if (url.username || url.password) throw new HttpError(400, 'credentials_in_url');
  if (url.port && url.port !== '443') throw new HttpError(400, 'port_not_allowed');
  const host = url.hostname.toLowerCase().replace(/\.$/, '');
  // IP literals (v6 contains ':'; v4 may be written as decimal, octal or hex), single-label and
  // internal names never leave the proxy.
  const labels = host.split('.');
  const isNumeric = /^[0-9.x]+$/i.test(host) || host.includes(':') || labels.some((l) => /^0x/i.test(l));
  if (isNumeric || labels.length < 2 || host === 'kortex.site' || BLOCKED_SUFFIXES.some((s) => host.endsWith(s))
    || !labels.every((l) => /^[a-z0-9-]{1,63}$/.test(l) && !l.startsWith('-') && !l.endsWith('-'))
    || !/^([a-z]{2,}|xn--[a-z0-9-]+)$/.test(labels[labels.length - 1]!)) {
    throw new HttpError(403, 'host_not_allowed');
  }
  url.hash = '';
  const connector = connectorHosts(env).some((c) => host === c || host.endsWith('.' + c));
  return { url, connector };
}

function checkHeaders(v: unknown, connector: boolean): Headers {
  const out = new Headers();
  if (v === undefined) return out;
  if (!isRecord(v) || Object.keys(v).length > 12) throw new HttpError(400, 'invalid_headers');
  const allowed = connector ? CONNECTOR_HEADERS : FREE_HEADERS;
  for (const [name, value] of Object.entries(v)) {
    const key = name.toLowerCase();
    if (typeof value !== 'string' || value.length > 2_000 || /[\r\n]/.test(value) || !/^[a-z0-9-]{1,40}$/.test(key)) {
      throw new HttpError(400, 'invalid_headers');
    }
    if (!allowed.has(key)) throw new HttpError(400, 'header_not_allowed');
    out.set(key, value);
  }
  return out;
}

function decodeBody(body: unknown, base64: unknown): Uint8Array | string | undefined {
  if (body === undefined && base64 === undefined) return undefined;
  if (typeof body === 'string' && base64 === undefined) {
    if (new TextEncoder().encode(body).byteLength > MAX_REQUEST_BODY) throw new HttpError(413, 'payload_too_large');
    return body;
  }
  if (typeof base64 === 'string' && body === undefined && /^[A-Za-z0-9+/]*={0,2}$/.test(base64) && base64.length <= MAX_REQUEST_BODY * 1.4) {
    const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
    if (bytes.byteLength > MAX_REQUEST_BODY) throw new HttpError(413, 'payload_too_large');
    return bytes;
  }
  throw new HttpError(400, 'invalid_body');
}

async function readLimited(res: Response, limit: number): Promise<Uint8Array> {
  const declared = Number(res.headers.get('content-length') ?? '0');
  if (declared > limit) { void res.body?.cancel().catch(() => {}); throw new HttpError(502, 'upstream_too_large'); }
  const reader = res.body?.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  if (reader) {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > limit) { void reader.cancel().catch(() => {}); throw new HttpError(502, 'upstream_too_large'); }
      chunks.push(value);
    }
  }
  const out = new Uint8Array(size);
  let offset = 0;
  for (const c of chunks) { out.set(c, offset); offset += c.length; }
  return out;
}

function toBase64(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

export async function forward(ctx: Ctx): Promise<Response> {
  const userId = await requireSession(ctx);
  const body = await readJson(ctx.req, 300_000);
  if (!isRecord(body)) throw new HttpError(400, 'invalid_request');
  let { url, connector } = checkTarget(body.url, ctx.env);
  const method = typeof body.method === 'string' ? body.method.toUpperCase() : 'GET';
  if (!(connector ? CONNECTOR_METHODS : FREE_METHODS).has(method)) throw new HttpError(403, 'method_not_allowed');
  const headers = checkHeaders(body.headers, connector);
  const payload = decodeBody(body.body, body.body_base64);
  if (payload !== undefined && (!connector || method === 'GET' || method === 'HEAD')) throw new HttpError(400, 'body_not_allowed');

  const perHour = Number((connector ? ctx.env.PROXY_CONNECTOR_PER_HOUR : ctx.env.PROXY_FREE_PER_HOUR) ?? (connector ? 120 : 30));
  await rateLimit(ctx, `proxy:${connector ? 'c' : 'f'}:${userId}`, perHour, 3600);
  await rateLimit(ctx, `proxy:burst:${userId}`, 20, 60);

  let current = method;
  let currentBody = payload;
  let res: Response | undefined;
  for (let hop = 0; hop <= MAX_REDIRECTS; hop++) {
    try {
      res = await fetch(url.toString(), {
        method: current,
        headers: new Headers({ ...Object.fromEntries(headers), 'user-agent': 'Kortex-Proxy/1' }),
        body: currentBody as BodyInit | undefined,
        redirect: 'manual',
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
    } catch (e) {
      const timedOut = e instanceof Error && (e.name === 'TimeoutError' || e.name === 'AbortError');
      throw new HttpError(timedOut ? 504 : 502, timedOut ? 'upstream_timeout' : 'upstream_unreachable');
    }
    const location = res.headers.get('location');
    if (res.status < 300 || res.status >= 400 || !location || res.status === 304) break;
    if (hop === MAX_REDIRECTS || !FREE_METHODS.has(current)) break; // returned as is: the caller decides
    void res.body?.cancel().catch(() => {});
    let next: Target;
    try { next = checkTarget(new URL(location, url).toString(), ctx.env); }
    catch { throw new HttpError(502, 'redirect_not_allowed'); }
    // Credentials never follow a redirect to another host, and a free target never gains them.
    if (next.url.hostname !== url.hostname) headers.delete('authorization'), headers.delete('x-api-key');
    if (!next.connector) { for (const k of [...headers.keys()]) if (!FREE_HEADERS.has(k)) headers.delete(k); }
    url = next.url;
    connector = next.connector;
    current = 'GET';
    currentBody = undefined;
  }
  if (!res) throw new HttpError(502, 'upstream_unreachable');
  const bytes = current === 'HEAD' ? new Uint8Array() : await readLimited(res, connector ? CONNECTOR_RESPONSE_BYTES : FREE_RESPONSE_BYTES);
  const outHeaders: Record<string, string> = {};
  for (const name of RESPONSE_HEADERS) {
    const v = res.headers.get(name);
    if (v) outHeaders[name] = v.slice(0, 1_000);
  }
  return json({ status: res.status, headers: outHeaders, url: url.toString(), body_base64: toBase64(bytes) });
}
