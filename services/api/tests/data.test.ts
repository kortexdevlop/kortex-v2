import { test } from 'node:test';
import assert from 'node:assert/strict';
import { call, makeEnv, signup } from './helpers.ts';

test('settings: default, save, read, and size cap', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  assert.deepEqual((await call(env, 'GET', '/v1/settings', undefined, { cookie })).data.data, {});
  assert.equal((await call(env, 'PUT', '/v1/settings', { data: { mode: 'pro', theme: 'dark' } }, { cookie })).status, 200);
  assert.deepEqual((await call(env, 'GET', '/v1/settings', undefined, { cookie })).data.data, { mode: 'pro', theme: 'dark' });
  const big = await call(env, 'PUT', '/v1/settings', { data: { blob: 'x'.repeat(20_000) } }, { cookie });
  assert.equal(big.status, 413);
  assert.equal((await call(env, 'PUT', '/v1/settings', { data: [1, 2] }, { cookie })).status, 400);
});

test('sync: create, update with version, list, delete', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  const c = await call(env, 'PUT', '/v1/sync/desk.layouts', { data: { spaces: [1] }, version: 0 }, { cookie });
  assert.deepEqual([c.status, c.data.version], [201, 1]);
  const u = await call(env, 'PUT', '/v1/sync/desk.layouts', { data: { spaces: [1, 2] }, version: 1 }, { cookie });
  assert.deepEqual([u.status, u.data.version], [200, 2]);
  const g = await call(env, 'GET', '/v1/sync/desk.layouts', undefined, { cookie });
  assert.deepEqual([g.data.version, g.data.data], [2, { spaces: [1, 2] }]);
  assert.deepEqual((await call(env, 'GET', '/v1/sync', undefined, { cookie })).data.items.map((i: any) => i.key), ['desk.layouts']);
  assert.equal((await call(env, 'DELETE', '/v1/sync/desk.layouts', undefined, { cookie })).status, 200);
  assert.equal((await call(env, 'GET', '/v1/sync/desk.layouts', undefined, { cookie })).status, 404);
});

test('sync: stale version and double-create return 409 with the current copy', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  await call(env, 'PUT', '/v1/sync/k', { data: { v: 'A' }, version: 0 }, { cookie });
  await call(env, 'PUT', '/v1/sync/k', { data: { v: 'B' }, version: 1 }, { cookie });
  const stale = await call(env, 'PUT', '/v1/sync/k', { data: { v: 'C' }, version: 1 }, { cookie });
  assert.equal(stale.status, 409);
  assert.equal(stale.data.error, 'version_conflict');
  assert.deepEqual([stale.data.current.version, stale.data.current.data], [2, { v: 'B' }]);
  const again = await call(env, 'PUT', '/v1/sync/k', { data: { v: 'D' }, version: 0 }, { cookie });
  assert.equal(again.status, 409);
  assert.equal(again.data.current.version, 2);
});

test('sync: validation of keys, sizes, shape and JSON', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  assert.equal((await call(env, 'PUT', '/v1/sync/Bad%20Key', { data: 1, version: 0 }, { cookie })).status, 400);
  assert.equal((await call(env, 'PUT', '/v1/sync/k', { data: 'x'.repeat(70_000), version: 0 }, { cookie })).status, 413);
  assert.equal((await call(env, 'PUT', '/v1/sync/k', { data: 1, version: -1 }, { cookie })).status, 400);
  assert.equal((await call(env, 'PUT', '/v1/sync/k', { version: 0 }, { cookie })).status, 400);
  assert.equal((await call(env, 'PUT', '/v1/sync/k', undefined, { cookie, raw: '{not json' })).status, 400);
});

test('sync: users are isolated from each other', async () => {
  const { env } = makeEnv();
  const a = await signup(env, 'a@example.com');
  const b = await signup(env, 'b@example.com');
  await call(env, 'PUT', '/v1/sync/secret', { data: { owner: 'a' }, version: 0 }, { cookie: a.cookie });
  assert.equal((await call(env, 'GET', '/v1/sync/secret', undefined, { cookie: b.cookie })).status, 404);
  assert.deepEqual((await call(env, 'GET', '/v1/sync', undefined, { cookie: b.cookie })).data.items, []);
  await call(env, 'PUT', '/v1/sync/secret', { data: { owner: 'b' }, version: 0 }, { cookie: b.cookie });
  assert.deepEqual((await call(env, 'GET', '/v1/sync/secret', undefined, { cookie: a.cookie })).data.data, { owner: 'a' });
});

test('usage: counters aggregate per day and tool, with validation', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  await call(env, 'POST', '/v1/usage', { tool: 'pdf.merge' }, { cookie });
  await call(env, 'POST', '/v1/usage', { tool: 'pdf.merge', count: 4 }, { cookie });
  await call(env, 'POST', '/v1/usage', { tool: 'qr.generator' }, { cookie });
  const r = await call(env, 'GET', '/v1/usage?days=7', undefined, { cookie });
  const by = Object.fromEntries(r.data.items.map((i: any) => [i.tool, i.count]));
  assert.deepEqual(by, { 'pdf.merge': 5, 'qr.generator': 1 });
  assert.equal((await call(env, 'POST', '/v1/usage', { tool: 'DROP TABLE' }, { cookie })).status, 400);
  assert.equal((await call(env, 'POST', '/v1/usage', { tool: 'pdf.merge', count: 0 }, { cookie })).status, 400);
  assert.equal((await call(env, 'POST', '/v1/usage', { tool: 'pdf.merge', count: 5000 }, { cookie })).status, 400);
});

test('sync: expected account rejects a cookie switch before reading or writing data', async () => {
  const { env } = makeEnv();
  const a = await signup(env, 'expected-a@example.com');
  const b = await signup(env, 'expected-b@example.com');
  const aid = (await call(env, 'GET', '/v1/me', undefined, { cookie: a.cookie })).data.id as string;
  const bid = (await call(env, 'GET', '/v1/me', undefined, { cookie: b.cookie })).data.id as string;
  const payload = {version: 0, data: {owner: 'a'}};
  assert.equal((await call(env, 'PUT', '/v1/sync/hub.layout.v1', payload, {cookie: a.cookie,headers:{'X-Kortex-User':aid}})).status,201);
  for(const method of ['GET','PUT']) {
    const changed=await call(env,method,'/v1/sync/hub.layout.v1',method==='PUT'?payload:undefined,{cookie:b.cookie,headers:{'X-Kortex-User':aid}});
    assert.deepEqual([changed.status,changed.data.error],[409,'session_changed']);
  }
  assert.equal((await call(env,'GET','/v1/sync/hub.layout.v1',undefined,{cookie:b.cookie,headers:{'X-Kortex-User':bid}})).status,404);
  const preflight=await call(env,'OPTIONS','/v1/sync/hub.layout.v1',undefined,{origin:'https://workspace.kortex.site'});
  assert.match(preflight.headers.get('access-control-allow-headers')??'',/X-Kortex-User/);
});
