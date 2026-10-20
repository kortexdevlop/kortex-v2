import test from 'node:test';
import assert from 'node:assert/strict';
import { barPercent, budgetTone, fmtDuration, rollbackCommand, summarizeErrors } from '../src/lib/admin-view.ts';

test('rollback commands are built only for known Workers and well-formed version ids', () => {
  const id = '651da94e-ea3f-4afe-b6b9-ce2634a2d5bc';
  assert.equal(rollbackCommand('kortex-api', id), `wrangler rollback ${id} --config services/api/wrangler.jsonc --env production`);
  assert.equal(rollbackCommand('kortex-flow', id), `wrangler rollback ${id} --config services/flow/wrangler.jsonc`);
  assert.equal(rollbackCommand('kortex-unknown', id), null);
  for (const bad of ['', 'not-a-version', `${id}; rm -rf /`, `${id.toUpperCase()}`]) assert.equal(rollbackCommand('kortex-api', bad), null, bad);
});

test('errors are grouped per app, biggest first, and bars never overflow', () => {
  const rows = [
    { day: '2026-10-20', app: 'flow', kind: 'panic', n: 4 },
    { day: '2026-10-19', app: 'flow', kind: 'unhandled', n: 2 },
    { day: '2026-10-20', app: 'agent', kind: 'panic', n: 1 },
    { day: '2026-10-20', app: 'store', kind: 'panic', n: 1 },
  ];
  assert.deepEqual(summarizeErrors(rows).map((e) => [e.app, e.total]), [['flow', 6], ['agent', 1], ['store', 1]]);
  assert.deepEqual(summarizeErrors(rows)[0].kinds, { panic: 4, unhandled: 2 });
  assert.deepEqual(summarizeErrors([]), []);
  assert.deepEqual([barPercent(4500, 9000), barPercent(20000, 9000), barPercent(5, 0), barPercent(-3, 9000)], [50, 100, 0, 0]);
  assert.deepEqual([budgetTone(10), budgetTone(70), budgetTone(95)], ['ok', 'warn', 'bad']);
  assert.deepEqual([fmtDuration(85), fmtDuration(2449)], ['85 ms', '2.4 s']);
});
