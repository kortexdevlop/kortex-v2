import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hashPassword, verifyPassword } from '../src/crypto.ts';
import { call, creds, makeEnv, signup } from './helpers.ts';

test('password hash: format, roundtrip, wrong password, different salts', async () => {
  const h = await hashPassword('s3cret-passphrase', 1000);
  assert.match(h, /^pbkdf2\$1000\$[A-Za-z0-9+/=]+\$[A-Za-z0-9+/=]+$/);
  assert.equal(await verifyPassword('s3cret-passphrase', h), true);
  assert.equal(await verifyPassword('s3cret-passphrasE', h), false);
  assert.notEqual(h, await hashPassword('s3cret-passphrase', 1000));
  assert.equal(await verifyPassword('x', 'garbage'), false);
});

test('register sets a host-only HttpOnly SameSite cookie and /me works', async () => {
  const { env } = makeEnv();
  const r = await signup(env);
  assert.equal(r.status, 201);
  const sc = r.headers.get('set-cookie') ?? '';
  assert.match(sc, /HttpOnly/);
  assert.match(sc, /Secure/);
  assert.match(sc, /SameSite=Lax/);
  assert.doesNotMatch(sc, /Domain=/i);
  const me = await call(env, 'GET', '/v1/me', undefined, { cookie: r.cookie });
  assert.equal(me.status, 200);
  assert.equal(me.data.email, 'a@example.com');
  assert.equal(me.data.password_hash, undefined);
});

test('registration validation and duplicate email', async () => {
  const { env } = makeEnv();
  const good = await creds('b@example.com', 'correct horse battery');
  assert.equal((await call(env, 'POST', '/v1/auth/register', { ...good, email: 'nope' })).data.error, 'invalid_email');
  assert.equal((await call(env, 'POST', '/v1/auth/register', { ...good, auth_key: 'short' })).data.error, 'invalid_auth_key');
  assert.equal((await call(env, 'POST', '/v1/auth/register', { ...good, auth_key: 'a'.repeat(44) })).data.error, 'invalid_auth_key');
  assert.equal((await call(env, 'POST', '/v1/auth/register', { ...good, kdf: 2 })).data.error, 'unsupported_kdf');
  assert.equal((await call(env, 'POST', '/v1/auth/register', { email: 'b@example.com', password: 'correct horse battery' })).data.error, 'unsupported_kdf');
  const { env: env2 } = makeEnv(); // fresh rate-limit window
  assert.equal((await signup(env2, 'dup@example.com')).status, 201);
  const again = await call(env2, 'POST', '/v1/auth/register', await creds('DUP@example.com', 'correct horse battery')); // case-insensitive
  assert.deepEqual([again.status, again.data.error], [409, 'email_taken']);
});

test('the server never stores or accepts a plaintext password', async () => {
  const { env, raw } = makeEnv();
  await signup(env, 'p@example.com', 'correct horse battery');
  const row = raw.prepare('SELECT password_hash FROM users').get() as { password_hash: string };
  assert.doesNotMatch(row.password_hash, /correct horse battery/);
  const key = (await creds('p@example.com', 'correct horse battery')).auth_key;
  assert.ok(!row.password_hash.includes(key), 'the derived key itself is hashed again before storage');
  assert.match(row.password_hash, /^pbkdf2\$1000\$/);
});

test('login: success, wrong password and unknown email are indistinguishable', async () => {
  const { env } = makeEnv();
  await signup(env, 'l@example.com', 'correct horse battery');
  const ok = await call(env, 'POST', '/v1/auth/login', await creds('L@example.com', 'correct horse battery'));
  assert.equal(ok.status, 200);
  assert.ok(ok.headers.get('set-cookie'));
  const bad = await call(env, 'POST', '/v1/auth/login', await creds('l@example.com', 'wrong password!!'));
  const none = await call(env, 'POST', '/v1/auth/login', await creds('ghost@example.com', 'wrong password!!'));
  assert.deepEqual([bad.status, bad.data], [401, { error: 'invalid_credentials' }]);
  assert.deepEqual([none.status, none.data], [401, { error: 'invalid_credentials' }]);
});

test('logout invalidates the session server-side', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  assert.equal((await call(env, 'POST', '/v1/auth/logout', {}, { cookie })).status, 200);
  assert.equal((await call(env, 'GET', '/v1/me', undefined, { cookie })).status, 401);
});

test('session ids are stored hashed and expired sessions are rejected', async () => {
  const { env, raw } = makeEnv();
  const { cookie } = await signup(env);
  const token = cookie.split('=')[1] ?? '';
  const rows = raw.prepare('SELECT id FROM sessions').all() as { id: string }[];
  assert.equal(rows.length, 1);
  assert.notEqual(rows[0]?.id, token);
  assert.match(rows[0]?.id ?? '', /^[0-9a-f]{64}$/);
  raw.exec('UPDATE sessions SET expires_at = 1');
  assert.equal((await call(env, 'GET', '/v1/me', undefined, { cookie })).status, 401);
  assert.equal((raw.prepare('SELECT COUNT(*) AS n FROM sessions').get() as { n: number }).n, 0);
});

test('unauthenticated access is refused everywhere that needs a user', async () => {
  const { env } = makeEnv();
  for (const [m, p] of [['GET', '/v1/me'], ['GET', '/v1/settings'], ['GET', '/v1/sync'], ['GET', '/v1/usage'], ['GET', '/v1/me/export']] as const) {
    assert.equal((await call(env, m, p)).status, 401, `${m} ${p}`);
  }
});

test('rate limiting: login is throttled per email and register per client', async () => {
  const { env } = makeEnv();
  for (let i = 0; i < 10; i++) await call(env, 'POST', '/v1/auth/login', await creds('victim@example.com', 'guess number ' + i));
  assert.equal((await call(env, 'POST', '/v1/auth/login', await creds('victim@example.com', 'guess number 11'))).status, 429);
  const { env: env2 } = makeEnv();
  for (let i = 0; i < 5; i++) await call(env2, 'POST', '/v1/auth/register', await creds(`u${i}@example.com`, 'correct horse battery'));
  assert.equal((await call(env2, 'POST', '/v1/auth/register', await creds('u9@example.com', 'correct horse battery'))).status, 429);
});

test('account deletion needs the password and removes all data (cascade)', async () => {
  const { env, raw } = makeEnv();
  const { cookie } = await signup(env);
  await call(env, 'PUT', '/v1/settings', { data: { theme: 'dark' } }, { cookie });
  await call(env, 'PUT', '/v1/sync/desk.layouts', { data: { a: 1 }, version: 0 }, { cookie });
  await call(env, 'POST', '/v1/usage', { tool: 'pdf.merge' }, { cookie });
  assert.equal((await call(env, 'DELETE', '/v1/me', await creds('a@example.com', 'wrong password!!'), { cookie })).status, 403);
  assert.equal((await call(env, 'DELETE', '/v1/me', await creds('a@example.com', 'correct horse battery'), { cookie })).status, 200);
  for (const t of ['users', 'sessions', 'settings', 'sync_items', 'usage_daily']) {
    assert.equal((raw.prepare(`SELECT COUNT(*) AS n FROM ${t}`).get() as { n: number }).n, 0, t);
  }
  assert.equal((await call(env, 'GET', '/v1/me', undefined, { cookie })).status, 401);
});

test('export returns the user data without any password hash', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  await call(env, 'PUT', '/v1/sync/k1', { data: { x: 1 }, version: 0 }, { cookie });
  const r = await call(env, 'GET', '/v1/me/export', undefined, { cookie });
  assert.equal(r.status, 200);
  assert.equal(r.data.user.email, 'a@example.com');
  assert.equal(r.data.sync.length, 1);
  assert.doesNotMatch(JSON.stringify(r.data), /pbkdf2|password/);
});
