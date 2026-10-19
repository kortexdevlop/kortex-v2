import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cleanup } from '../src/index.ts';
import { call, makeEnv } from './helpers.ts';

const rows = (raw: ReturnType<typeof makeEnv>['raw']) => raw.prepare('SELECT day, app, kind, n FROM client_errors ORDER BY app, kind').all() as { day: string; app: string; kind: string; n: number }[];

test('a report only bumps a daily counter and nothing identifying is stored', async () => {
  const { env, raw } = makeEnv();
  const origin = 'https://workspace.kortex.site';
  for (let i = 0; i < 3; i++) {
    const r = await call(env, 'POST', '/v1/telemetry', { app: 'flow', kind: 'panic' }, { origin: 'https://kortex.site' });
    assert.equal(r.status, 204);
  }
  assert.equal((await call(env, 'POST', '/v1/telemetry', { app: 'flow', kind: 'unhandled' }, { origin: 'https://kortex.site' })).status, 204);
  assert.equal((await call(env, 'POST', '/v1/telemetry', { app: 'agent', kind: 'panic' }, { origin })).status, 204);
  const stored = rows(raw);
  assert.deepEqual(stored.map(r => [r.app, r.kind, r.n]), [['agent', 'panic', 1], ['flow', 'panic', 3], ['flow', 'unhandled', 1]]);
  // The table has no column that could identify a person or a page.
  const columns = (raw.prepare("SELECT name FROM pragma_table_info('client_errors')").all() as { name: string }[]).map(c => c.name);
  assert.deepEqual(columns.sort(), ['app', 'day', 'kind', 'n']);
  assert.ok(!JSON.stringify(raw.prepare('SELECT * FROM rate_limits').all()).includes('local'), 'the address is hashed in the limiter');
});

test('unknown apps, kinds and shapes are refused and cross-site posts are blocked', async () => {
  const { env, raw } = makeEnv();
  for (const body of [{ app: 'evil', kind: 'panic' }, { app: 'flow', kind: 'message with <script>' }, { app: 'flow' }, { app: 1, kind: 2 }, { app: 'flow', kind: 'panic', message: 'x'.repeat(5000) }]) {
    const r = await call(env, 'POST', '/v1/telemetry', body, { origin: 'https://kortex.site' });
    assert.ok([400, 413].includes(r.status), JSON.stringify(body).slice(0, 60));
  }
  assert.equal((await call(env, 'POST', '/v1/telemetry', { app: 'flow', kind: 'panic' }, { origin: 'https://evil.example' })).status, 403);
  assert.equal(rows(raw).length, 0);
});

test('reports are rate limited per address and old counters are swept', async () => {
  const { env, raw } = makeEnv();
  let last = 204;
  for (let i = 0; i < 32; i++) last = (await call(env, 'POST', '/v1/telemetry', { app: 'flow', kind: 'panic' }, { origin: 'https://kortex.site' })).status;
  assert.equal(last, 429);
  raw.prepare("INSERT INTO client_errors (day, app, kind, n) VALUES ('2020-01-01', 'flow', 'panic', 9)").run();
  await cleanup(env);
  assert.ok(rows(raw).every(r => r.day !== '2020-01-01'), 'counters older than 90 days are deleted');
});
