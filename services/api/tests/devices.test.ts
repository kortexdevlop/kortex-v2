import { test } from 'node:test';
import assert from 'node:assert/strict';
import { call, creds, makeEnv, signup } from './helpers.ts';
import type { Env } from '../src/env.ts';

/** CLI-style call: no Origin, a device token, and the CSRF proof header. */
const dcall = (env: Env, method: string, path: string, body: unknown, token: string) =>
  call(env, method, path, body, { origin: null, headers: { Authorization: `Bearer ${token}`, 'X-Kortex-Request': '1' } });

async function device(env: Env, email = 'a@example.com', password = 'correct horse battery', name = 'MacBook') {
  const r = await call(env, 'POST', '/v1/auth/device', { ...(await creds(email, password)), name, platform: 'macos-aarch64' }, { origin: null, headers: { 'X-Kortex-Request': '1' } });
  return { ...r, token: (r.data?.token as string) ?? '', id: (r.data?.device_id as string) ?? '' };
}

test('device login returns a token once, stored only as a hash, and it authenticates', async () => {
  const { env, raw } = makeEnv();
  await signup(env);
  const d = await device(env);
  assert.equal(d.status, 201);
  assert.match(d.token, /^kdt_[A-Za-z0-9_-]{43}$/);
  const row = raw.prepare('SELECT token_hash FROM devices').get() as { token_hash: string };
  assert.notEqual(row.token_hash, d.token);
  assert.match(row.token_hash, /^[0-9a-f]{64}$/);
  const me = await dcall(env, 'GET', '/v1/me', undefined, d.token);
  assert.deepEqual([me.status, me.data.email], [200, 'a@example.com']);
});

test('bad credentials and unknown accounts are indistinguishable, and share the login rate limit', async () => {
  const { env } = makeEnv();
  await signup(env);
  const bad = await device(env, 'a@example.com', 'wrong password!!');
  const none = await device(env, 'ghost@example.com', 'wrong password!!');
  assert.deepEqual([bad.status, bad.data], [401, { error: 'invalid_credentials' }]);
  assert.deepEqual([none.status, none.data], [401, { error: 'invalid_credentials' }]);
  for (let i = 0; i < 10; i++) await device(env, 'a@example.com', 'guess ' + i + ' xxxxx');
  assert.equal((await device(env)).status, 429, 'the device endpoint is not a side door around login throttling');
});

test('a device token has a limited scope: data yes, account deletion no', async () => {
  const { env } = makeEnv();
  const s = await signup(env);
  const d = await device(env);
  assert.equal((await dcall(env, 'PUT', '/v1/settings', { data: { mode: 'pro' } }, d.token)).status, 200);
  assert.deepEqual((await dcall(env, 'GET', '/v1/settings', undefined, d.token)).data.data, { mode: 'pro' });
  assert.equal((await dcall(env, 'PUT', '/v1/sync/k', { data: 1, version: 0 }, d.token)).status, 201);
  assert.equal((await dcall(env, 'POST', '/v1/usage', { tool: 'pdf.merge' }, d.token)).status, 200);
  assert.equal((await dcall(env, 'GET', '/v1/ai/credits', undefined, d.token)).status, 200);
  const del = await dcall(env, 'DELETE', '/v1/me', await creds('a@example.com', 'correct horse battery'), d.token);
  assert.equal(del.status, 401, 'deleting the account needs the browser session, never a stolen device token');
  assert.equal((await call(env, 'GET', '/v1/me', undefined, { cookie: s.cookie })).status, 200, 'account still there');
});

test('malformed, unknown and revoked tokens are refused', async () => {
  const { env } = makeEnv();
  await signup(env);
  const d = await device(env);
  for (const t of ['', 'x', 'kdt_' + 'A'.repeat(43), d.token + 'x']) assert.equal((await dcall(env, 'GET', '/v1/me', undefined, t)).status, 401, t);
  const noScheme = await call(env, 'GET', '/v1/me', undefined, { headers: { Authorization: d.token } });
  assert.equal(noScheme.status, 401);
  assert.equal((await dcall(env, 'DELETE', '/v1/devices/self', undefined, d.token)).status, 200);
  assert.equal((await dcall(env, 'GET', '/v1/me', undefined, d.token)).status, 401, 'revoked');
});

test('device list, revoke by session, and isolation between users', async () => {
  const { env } = makeEnv();
  const a = await signup(env, 'a@example.com');
  const b = await signup(env, 'b@example.com');
  const da = await device(env, 'a@example.com', 'correct horse battery', 'Laptop');
  const db = await device(env, 'b@example.com', 'correct horse battery', 'Desktop');
  const list = await call(env, 'GET', '/v1/devices', undefined, { cookie: a.cookie });
  assert.deepEqual(list.data.devices.map((x: any) => x.name), ['Laptop']);
  assert.equal(JSON.stringify(list.data).includes('token'), false, 'tokens and hashes are never listed');
  assert.equal((await call(env, 'DELETE', `/v1/devices/${db.id}`, undefined, { cookie: a.cookie })).status, 404, "cannot revoke someone else's device");
  assert.equal((await dcall(env, 'GET', '/v1/me', undefined, db.token)).status, 200);
  assert.equal((await call(env, 'DELETE', `/v1/devices/${da.id}`, undefined, { cookie: a.cookie })).status, 200);
  assert.equal((await dcall(env, 'GET', '/v1/me', undefined, da.token)).status, 401);
  assert.equal((await dcall(env, 'DELETE', `/v1/devices/${db.id}`, undefined, db.token)).status, 401, 'a device cannot revoke others by id');
  void b;
});

test('at most 20 devices per account', async () => {
  const { env, raw } = makeEnv();
  await signup(env);
  const uid = (raw.prepare('SELECT id FROM users').get() as { id: string }).id;
  for (let i = 0; i < 20; i++) {
    raw.prepare('INSERT INTO devices (id, user_id, name, token_hash, created_at, last_seen) VALUES (?1, ?2, ?3, ?4, 1, 1)').run(`d${i}`, uid, `device ${i}`, `hash${i}`);
  }
  const r = await device(env);
  assert.deepEqual([r.status, r.data.error], [409, 'too_many_devices']);
  raw.exec("DELETE FROM devices WHERE id = 'd0'");
  assert.equal((await device(env)).status, 201, 'a slot frees up when a device is revoked');
});

test('confirm re-checks the password and is throttled', async () => {
  const { env } = makeEnv();
  await signup(env);
  const d = await device(env);
  const good = await creds('a@example.com', 'correct horse battery');
  assert.equal((await dcall(env, 'POST', '/v1/auth/confirm', good, d.token)).status, 200);
  const bad = await creds('a@example.com', 'not the password!!');
  assert.deepEqual((await dcall(env, 'POST', '/v1/auth/confirm', bad, d.token)).status, 403);
  for (let i = 0; i < 8; i++) await dcall(env, 'POST', '/v1/auth/confirm', bad, d.token);
  assert.equal((await dcall(env, 'POST', '/v1/auth/confirm', good, d.token)).status, 429);
  assert.equal((await call(env, 'POST', '/v1/auth/confirm', good)).status, 401, 'needs authentication');
});

test('installs: record, upsert, validate, list per device, remove', async () => {
  const { env } = makeEnv();
  const s = await signup(env);
  const d = await device(env, 'a@example.com', 'correct horse battery', 'Laptop');
  const rec = (v: string) => dcall(env, 'POST', '/v1/installs', { tool: 'pdf.merge', version: v, target: 'aarch64-apple-darwin' }, d.token);
  assert.equal((await rec('1.0.0')).status, 200);
  assert.equal((await rec('1.1.0')).status, 200, 'same tool on same device: upsert');
  const list = await call(env, 'GET', '/v1/installs', undefined, { cookie: s.cookie });
  assert.deepEqual(list.data.items.map((i: any) => [i.tool, i.version, i.device_name]), [['pdf.merge', '1.1.0', 'Laptop']]);
  for (const bad of [{ tool: 'DROP TABLE', version: '1.0.0', target: 'aarch64-apple-darwin' }, { tool: 'pdf.merge', version: 'latest', target: 'aarch64-apple-darwin' }, { tool: 'pdf.merge', version: '1.0.0', target: '../x' }]) {
    assert.equal((await dcall(env, 'POST', '/v1/installs', bad, d.token)).status, 400, JSON.stringify(bad));
  }
  assert.equal((await call(env, 'POST', '/v1/installs', { tool: 'pdf.merge', version: '1.0.0', target: 'aarch64-apple-darwin' }, { cookie: s.cookie })).data.error, 'device_required', 'a browser session has no device to attach installs to');
  assert.equal((await dcall(env, 'DELETE', '/v1/installs/pdf.merge', undefined, d.token)).status, 200);
  assert.deepEqual((await call(env, 'GET', '/v1/installs', undefined, { cookie: s.cookie })).data.items, []);
});

test('installs are private per user and disappear with the device and with the account', async () => {
  const { env, raw } = makeEnv();
  const a = await signup(env, 'a@example.com');
  const b = await signup(env, 'b@example.com');
  const da = await device(env, 'a@example.com');
  await dcall(env, 'POST', '/v1/installs', { tool: 'qr.generator', version: '1.0.0', target: 'x86_64-pc-windows-msvc' }, da.token);
  assert.deepEqual((await call(env, 'GET', '/v1/installs', undefined, { cookie: b.cookie })).data.items, []);
  await call(env, 'DELETE', `/v1/devices/${da.id}`, undefined, { cookie: a.cookie });
  assert.equal((raw.prepare('SELECT COUNT(*) AS n FROM installs').get() as { n: number }).n, 0, 'cascade on device revoke');
  const da2 = await device(env, 'a@example.com');
  await dcall(env, 'POST', '/v1/installs', { tool: 'qr.generator', version: '1.0.0', target: 'x86_64-pc-windows-msvc' }, da2.token);
  await call(env, 'DELETE', '/v1/me', await creds('a@example.com', 'correct horse battery'), { cookie: a.cookie });
  for (const t of ['devices', 'installs']) assert.equal((raw.prepare(`SELECT COUNT(*) AS n FROM ${t}`).get() as { n: number }).n, 0, t);
});

test('export includes devices and installs, without any token material', async () => {
  const { env } = makeEnv();
  const s = await signup(env);
  const d = await device(env);
  await dcall(env, 'POST', '/v1/installs', { tool: 'pdf.merge', version: '1.0.0', target: 'aarch64-apple-darwin' }, d.token);
  const r = await call(env, 'GET', '/v1/me/export', undefined, { cookie: s.cookie });
  assert.equal(r.data.devices?.length, 1);
  assert.equal(r.data.installs?.length, 1);
  assert.doesNotMatch(JSON.stringify(r.data), /kdt_|token_hash/);
});
