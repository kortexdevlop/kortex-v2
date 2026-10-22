import { test } from 'node:test';
import assert from 'node:assert/strict';
import { call, makeEnv, signup } from './helpers.ts';

const realFetch = globalThis.fetch;
function hostsAnswer(down: string[] = []) {
  globalThis.fetch = (async (input: RequestInfo | URL) => {
    const host = new URL(String(input)).hostname;
    if (down.includes(host)) throw new Error('down');
    return new Response(host === 'api.kortex.site' ? JSON.stringify({ ok: true }) : null, { status: host.startsWith('run.') ? 404 : 200 });
  }) as typeof fetch;
}
const restore = () => { globalThis.fetch = realFetch; };

test('a missing app is unhealthy and the API probe uses its health endpoint', async () => {
  const { env } = makeEnv({ ALLOWED_ORIGINS: 'https://kortex.site,https://run.kortex.site' });
  const owner = await signup(env);
  env.ADMIN_USER_IDS = owner.data.id;
  const seen: string[] = [];
  globalThis.fetch = (async (input: RequestInfo | URL) => {
    const url = String(input); seen.push(url);
    return url.endsWith('/v1/health') ? new Response('{"ok":true}') : new Response(null, { status: 404 });
  }) as typeof fetch;
  try {
    const result = await call(env, 'GET', '/v1/admin/overview', undefined, { cookie: owner.cookie });
    assert.equal(result.status, 200);
    assert.ok(seen.includes('https://api.kortex.site/v1/health'));
    assert.equal(result.data.hosts.find((h: {host:string}) => h.host === 'run.kortex.site').ok, false);
  } finally { restore(); }
});

test('the dashboard does not exist until an owner is configured, and only that owner sees it', async () => {
  hostsAnswer();
  try {
    const open = makeEnv();
    const { cookie } = await signup(open.env, 'owner@example.com');
    assert.equal((await call(open.env, 'GET', '/v1/admin/overview', undefined, { cookie })).status, 404, 'no provisioned owner: a plain 404');
    const { env } = makeEnv({ ADMIN_EMAILS: 'owner@example.com' });
    const owner = await signup(env, 'owner@example.com');
    assert.equal((await call(env, 'GET', '/v1/admin/overview', undefined, { cookie: owner.cookie })).status, 404, 'claiming the legacy email grants no role');
    env.ADMIN_USER_IDS = ` ${owner.data.id} , another-account-id `;
    const visitor = await signup(env, 'visitor@example.com');
    assert.equal((await call(env, 'GET', '/v1/admin/overview')).status, 401, 'signed out');
    assert.equal((await call(env, 'GET', '/v1/admin/overview', undefined, { cookie: visitor.cookie })).status, 404, 'a normal account sees nothing');
    const ok = await call(env, 'GET', '/v1/admin/overview', undefined, { cookie: owner.cookie });
    assert.equal(ok.status, 200);
  } finally { restore(); }
});

test('the overview gathers health, errors, AI, database and releases without anything identifying', async () => {
  hostsAnswer(['agent.kortex.site']);
  try {
    const { env, raw } = makeEnv({ ADMIN_EMAILS: 'owner@example.com', ALLOWED_ORIGINS: 'https://kortex.site,https://flow.kortex.site,https://agent.kortex.site,http://localhost:4321' });
    const { cookie } = await signup(env, 'owner@example.com');
    env.ADMIN_USER_IDS = (raw.prepare('SELECT id FROM users').get() as { id: string }).id;
    const today = new Date().toISOString().slice(0, 10);
    raw.prepare("INSERT INTO client_errors (day, app, kind, n) VALUES (?1, 'flow', 'panic', 4), (?1, 'agent', 'unhandled', 1), ('2000-01-01', 'flow', 'panic', 99)").run(today);
    const r = await call(env, 'GET', '/v1/admin/overview', undefined, { cookie });
    assert.equal(r.status, 200);
    const d = r.data;
    assert.deepEqual(d.hosts.map((h: { host: string; ok: boolean }) => [h.host, h.ok]).sort(), [['agent.kortex.site', false], ['api.kortex.site', true], ['flow.kortex.site', true], ['kortex.site', true]]);
    assert.ok(d.hosts.every((h: { status: number }) => typeof h.status === 'number'));
    assert.deepEqual(d.errors.map((e: { app: string; n: number }) => [e.app, e.n]), [['agent', 1], ['flow', 4]], 'only the last seven days');
    assert.equal(d.ai.global.cap, 9000);
    assert.equal(d.database.users, 1);
    assert.ok(Array.isArray(d.releases) && d.releases.length > 0 && d.releases[0].versions, 'the publication journal');
    assert.equal(d.api_version, null, 'no version binding in tests');
    const text = JSON.stringify(d);
    assert.ok(!text.includes('owner@example.com'), 'no e-mail in the dashboard data');
  } finally { restore(); }
});
