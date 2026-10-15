import { test } from 'node:test';
import assert from 'node:assert/strict';
import { call, creds, makeEnv, signup } from './helpers.ts';

test('CSRF: writes from unknown origins or without proof of intent are refused', async () => {
  const { env } = makeEnv();
  const body = await creds('a@example.com', 'x'.repeat(12));
  const evil = await call(env, 'POST', '/v1/auth/login', body, { origin: 'https://evil.example' });
  assert.deepEqual([evil.status, evil.data.error], [403, 'forbidden_origin']);
  const noOrigin = await call(env, 'POST', '/v1/auth/login', body, { origin: null });
  assert.deepEqual([noOrigin.status, noOrigin.data.error], [403, 'missing_csrf_header']);
  const cli = await call(env, 'POST', '/v1/auth/login', body, { origin: null, headers: { 'X-Kortex-Request': '1' } });
  assert.equal(cli.status, 401); // reached the handler: just wrong credentials
});

test('CORS: only allow-listed origins are reflected, with credentials', async () => {
  const { env } = makeEnv();
  const ok = await call(env, 'GET', '/v1/health', undefined, { origin: 'https://workspace.kortex.site' });
  assert.equal(ok.headers.get('access-control-allow-origin'), 'https://workspace.kortex.site');
  assert.equal(ok.headers.get('access-control-allow-credentials'), 'true');
  const no = await call(env, 'GET', '/v1/health', undefined, { origin: 'https://evil.example' });
  assert.equal(no.headers.get('access-control-allow-origin'), null);
  const pre = await call(env, 'OPTIONS', '/v1/settings', undefined, { origin: 'https://kortex.site' });
  assert.equal(pre.status, 204);
  assert.match(pre.headers.get('access-control-allow-headers') ?? '', /X-Kortex-Request/);
  assert.equal((await call(env, 'OPTIONS', '/v1/settings', undefined, { origin: 'https://evil.example' })).status, 403);
});

test('security headers and no-store on every response', async () => {
  const { env } = makeEnv();
  const r = await call(env, 'GET', '/v1/health');
  assert.equal(r.headers.get('cache-control'), 'no-store');
  assert.equal(r.headers.get('x-content-type-options'), 'nosniff');
  assert.match(r.headers.get('strict-transport-security') ?? '', /max-age=/);
});

test('unknown routes are 404, and bad input never becomes a 500', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  assert.equal((await call(env, 'GET', '/v1/nope')).status, 404);
  for (const raw of ['', '{', 'null', '[]', '"str"', '{"email":1,"password":2}']) {
    const r = await call(env, 'POST', '/v1/auth/login', undefined, { raw });
    assert.ok(r.status >= 400 && r.status < 500, `login with ${JSON.stringify(raw)} -> ${r.status}`);
  }
  const big = await call(env, 'POST', '/v1/auth/register', undefined, { raw: JSON.stringify({ email: 'a@b.co', auth_key: 'x'.repeat(10_000), kdf: 1 }) });
  assert.equal(big.status, 413);
  void cookie;
});

test('SQL-injection shaped input is treated as data', async () => {
  const { env, raw } = makeEnv();
  const { cookie } = await signup(env);
  const r = await call(env, 'PUT', "/v1/sync/k", { data: { q: "'; DROP TABLE users; --" }, version: 0 }, { cookie });
  assert.equal(r.status, 201);
  assert.equal((raw.prepare('SELECT COUNT(*) AS n FROM users').get() as { n: number }).n, 1);
  assert.equal((await call(env, 'POST', '/v1/auth/login', { ...(await creds('x@y.co', 'whatever-long')), email: "x@y.co' OR '1'='1" })).status, 400);
});
