import { test } from 'node:test';
import assert from 'node:assert/strict';
import { call, makeEnv, signup } from './helpers.ts';

type Seen = { url: string; init: RequestInit };
const realFetch = globalThis.fetch;

/** Replaces the network: `reply` decides the answer per request; every request is recorded. */
function network(reply: (url: string, init: RequestInit) => Response | Promise<Response>) {
  const seen: Seen[] = [];
  globalThis.fetch = (async (input: RequestInfo | URL, init: RequestInit = {}) => {
    const url = String(input);
    seen.push({ url, init });
    return reply(url, init);
  }) as typeof fetch;
  return seen;
}
const restore = () => { globalThis.fetch = realFetch; };
const text = (b64: string) => Buffer.from(b64, 'base64').toString('utf8');

async function user(extra = {}) {
  const { env } = makeEnv(extra);
  const { cookie } = await signup(env);
  return { env, cookie };
}

test('the proxy needs a signed-in user and the CSRF proof', async () => {
  const { env } = makeEnv();
  assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/' })).status, 401);
  const { cookie } = await signup(env);
  assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/' }, { cookie, origin: 'https://evil.example' })).status, 403);
});

test('private, internal, non-HTTPS and tricky destinations never leave the proxy', async () => {
  const { env, cookie } = await user();
  const seen = network(() => new Response('should not be called'));
  try {
    const refused: [string, number, string][] = [
      ['http://example.org/', 400, 'https_only'],
      ['https://127.0.0.1/', 403, 'host_not_allowed'],
      ['https://[::1]/', 403, 'host_not_allowed'],
      ['https://169.254.169.254/latest/meta-data', 403, 'host_not_allowed'],
      ['https://2130706433/', 403, 'host_not_allowed'],
      ['https://0x7f.1/', 403, 'host_not_allowed'],
      ['https://localhost/', 403, 'host_not_allowed'],
      ['https://intranet/', 403, 'host_not_allowed'],
      ['https://db.internal/', 403, 'host_not_allowed'],
      ['https://printer.local/', 403, 'host_not_allowed'],
      ['https://kortex.site/v1/me', 403, 'host_not_allowed'],
      ['https://api.kortex.site/v1/me', 403, 'host_not_allowed'],
      ['https://example.org:8443/', 400, 'port_not_allowed'],
      ['https://user:pw@example.org/', 400, 'credentials_in_url'],
      ['not a url', 400, 'invalid_url'],
    ];
    for (const [url, status, code] of refused) {
      const r = await call(env, 'POST', '/v1/proxy', { url }, { cookie });
      assert.deepEqual([r.status, r.data?.error], [status, code], url);
    }
    assert.equal(seen.length, 0, 'nothing was fetched');
  } finally { restore(); }
});

test('free mode: public GET only, no credentials, bounded headers', async () => {
  const { env, cookie } = await user();
  const seen = network(() => new Response('hello', { status: 200, headers: { 'content-type': 'text/plain', 'set-cookie': 'a=b', 'x-secret': 'no' } }));
  try {
    const ok = await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/page?q=1#frag', headers: { Accept: 'text/plain' } }, { cookie });
    assert.equal(ok.status, 200);
    assert.equal(ok.data.status, 200);
    assert.equal(text(ok.data.body_base64), 'hello');
    assert.deepEqual(Object.keys(ok.data.headers), ['content-type'], 'cookies and unknown headers are not relayed');
    assert.equal(seen[0]!.url, 'https://example.org/page?q=1');
    assert.equal(new Headers(seen[0]!.init.headers).get('cookie'), null, 'the session cookie never goes out');
    assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/', method: 'POST', body: 'x' }, { cookie })).data.error, 'method_not_allowed');
    assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/', headers: { Authorization: 'Bearer x' } }, { cookie })).data.error, 'header_not_allowed');
    assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/', headers: { Accept: 'a\r\nHost: evil' } }, { cookie })).data.error, 'invalid_headers');
  } finally { restore(); }
});

test('connector mode forwards the user credential and a body to an allow-listed API', async () => {
  const { env, cookie } = await user();
  const seen = network(() => new Response('{"ok":true}', { status: 201, headers: { 'content-type': 'application/json' } }));
  try {
    const r = await call(env, 'POST', '/v1/proxy', {
      url: 'https://api.github.com/repos/o/r/issues', method: 'post',
      headers: { Authorization: 'Bearer user-token', 'Content-Type': 'application/json', 'X-Evil': '1' },
      body: '{"title":"t"}',
    }, { cookie });
    assert.equal(r.data.error, 'header_not_allowed', 'unknown headers are refused even for connectors');
    const ok = await call(env, 'POST', '/v1/proxy', {
      url: 'https://api.github.com/repos/o/r/issues', method: 'post',
      headers: { Authorization: 'Bearer user-token', 'Content-Type': 'application/json' },
      body: '{"title":"t"}',
    }, { cookie });
    assert.equal(ok.status, 200);
    assert.equal(ok.data.status, 201);
    const sent = seen.at(-1)!;
    assert.equal(sent.init.method, 'POST');
    assert.equal(new Headers(sent.init.headers).get('authorization'), 'Bearer user-token');
    assert.equal(sent.init.body, '{"title":"t"}');
    // A body on a GET, and an oversized body, are refused.
    assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://api.github.com/', body: 'x' }, { cookie })).data.error, 'body_not_allowed');
    assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://api.github.com/', method: 'POST', body: 'x'.repeat(250_000) }, { cookie })).status, 413);
    // A look-alike host is not a connector.
    const fake = await call(env, 'POST', '/v1/proxy', { url: 'https://api.github.com.evil.org/', headers: { Authorization: 'x' } }, { cookie });
    assert.equal(fake.data.error, 'header_not_allowed');
  } finally { restore(); }
});

test('redirects are re-checked: private targets are refused, credentials stay on their host', async () => {
  const { env, cookie } = await user();
  let step = 0;
  const seen = network((url) => {
    step++;
    if (url.startsWith('https://api.github.com/a')) return new Response(null, { status: 302, headers: { location: 'https://example.org/landing' } });
    if (url.startsWith('https://api.github.com/b')) return new Response(null, { status: 302, headers: { location: 'https://169.254.169.254/' } });
    return new Response('landed', { status: 200 });
  });
  try {
    const ok = await call(env, 'POST', '/v1/proxy', { url: 'https://api.github.com/a', headers: { Authorization: 'Bearer t' } }, { cookie });
    assert.equal(text(ok.data.body_base64), 'landed');
    assert.equal(ok.data.url, 'https://example.org/landing');
    assert.equal(new Headers(seen[1]!.init.headers).get('authorization'), null, 'the token did not follow the redirect');
    const refused = await call(env, 'POST', '/v1/proxy', { url: 'https://api.github.com/b' }, { cookie });
    assert.equal(refused.data.error, 'redirect_not_allowed');
    assert.ok(step >= 3 && !seen.some((s) => s.url.includes('169.254')), 'the private address was never requested');
  } finally { restore(); }
});

test('big answers, slow servers and unreachable hosts become typed errors', async () => {
  const { env, cookie } = await user();
  try {
    network(() => new Response(new Uint8Array(1_500_000)));
    assert.deepEqual((({ status, data }) => [status, data.error])(await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/big' }, { cookie })), [502, 'upstream_too_large']);
    network(() => { throw Object.assign(new Error('t'), { name: 'TimeoutError' }); });
    assert.deepEqual((({ status, data }) => [status, data.error])(await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/slow' }, { cookie })), [504, 'upstream_timeout']);
    network(() => { throw new Error('dns'); });
    assert.deepEqual((({ status, data }) => [status, data.error])(await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/down' }, { cookie })), [502, 'upstream_unreachable']);
  } finally { restore(); }
});

test('the proxy is rate-limited per user and per kind of request', async () => {
  const { env, cookie } = await user({ PROXY_FREE_PER_HOUR: '3' });
  network(() => new Response('x'));
  try {
    for (let i = 0; i < 3; i++) assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/' }, { cookie })).status, 200);
    assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://example.org/' }, { cookie })).status, 429);
    // Connector calls have their own budget.
    assert.equal((await call(env, 'POST', '/v1/proxy', { url: 'https://api.github.com/' }, { cookie })).status, 200);
  } finally { restore(); }
});
