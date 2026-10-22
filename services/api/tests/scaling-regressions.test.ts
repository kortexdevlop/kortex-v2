import { test } from 'node:test';
import assert from 'node:assert/strict';
import { reserve, settle } from '../src/credits.ts';
import { call, makeEnv, signup } from './helpers.ts';

test('a concurrent debit sharing the timestamp is not overwritten by a stale reservation', async () => {
  const { env, raw } = makeEnv();
  const user = await signup(env);
  const now = Date.now();
  raw.prepare('INSERT INTO ai_buckets(user_id,tokens,updated_at) VALUES(?,1000,?)').run(user.data.id, now);
  const prepare = env.DB.prepare.bind(env.DB);
  let injected = false;
  env.DB.prepare = (sql: string) => {
    const statement = prepare(sql);
    if (sql.startsWith('UPDATE ai_buckets SET tokens = ?2')) {
      const bind = statement.bind.bind(statement);
      statement.bind = (...args: unknown[]) => {
        const bound = bind(...args);
        const run = bound.run.bind(bound);
        bound.run = async () => {
          if (!injected) {
            injected = true;
            raw.prepare('UPDATE ai_buckets SET tokens=900 WHERE user_id=?').run(user.data.id);
          }
          return run();
        };
        return bound;
      };
    }
    return statement;
  };
  assert.equal((await reserve(env, user.data.id, 100, now)).ok, true);
  const balance = raw.prepare('SELECT tokens FROM ai_buckets').get() as { tokens: number };
  assert.equal(balance.tokens, 800);
  assert.equal((raw.prepare('SELECT used FROM ai_global').get() as { used: number }).used, 100, 'lost CAS reservation refunded before retry');
});

test('a call crossing midnight settles the original reservation day', async () => {
  const { env, raw } = makeEnv();
  const user = await signup(env);
  const before = Date.UTC(2026, 9, 22, 23, 59, 59);
  const receipt = await reserve(env, user.data.id, 100, before);
  assert.ok(receipt.ok);
  await settle(env, user.data.id, receipt.cost, 20, 'gemma', before + 2000, 'flow', receipt.day);
  assert.deepEqual({ ...raw.prepare('SELECT day,used FROM ai_global').get() }, { day: '2026-10-22', used: 20 });
  assert.deepEqual({ ...raw.prepare('SELECT day,requests,neurons FROM ai_user_day').get() }, { day: '2026-10-22', requests: 1, neurons: 20 });
});

test('invalid proxy base64 and inherited model/provider names fail before network or quotas', async () => {
  const { env, raw } = makeEnv({ AI: { async run() { throw new Error('must not call provider'); } } as unknown as Ai });
  const user = await signup(env);
  const opts = { cookie: user.cookie };
  const proxy = await call(env, 'POST', '/v1/proxy', { url: 'https://api.github.com/test', method: 'POST', body_base64: 'A' }, opts);
  assert.deepEqual([proxy.status, proxy.data.error], [400, 'invalid_body']);
  for (const name of ['toString', '__proto__', 'constructor']) {
    const model = await call(env, 'POST', '/v1/ai/chat', { messages: [{ role: 'user', content: 'hello' }], consent: true, model: name }, opts);
    assert.deepEqual([model.status, model.data.error], [400, 'invalid_model']);
    const oauth = await call(env, 'POST', '/v1/oauth/exchange', { provider: name }, opts);
    assert.deepEqual([oauth.status, oauth.data.error], [400, 'unknown_provider']);
  }
  assert.equal((raw.prepare('SELECT count(*) n FROM ai_buckets').get() as { n: number }).n, 0);
});

test('AI route carries the reservation day into completion across midnight', async () => {
  const { env, raw } = makeEnv();
  const user = await signup(env);
  const originalNow = Date.now;
  const before = Date.UTC(2026, 9, 22, 23, 59, 59);
  Date.now = () => before;
  env.AI = { async run() {
    Date.now = () => before + 2000;
    return { choices: [{ message: { content: 'ok' } }], usage: { neurons: 1 } };
  } } as unknown as Ai;
  try {
    const result = await call(env, 'POST', '/v1/ai/chat', { messages: [{ role: 'user', content: 'hello' }], consent: true }, { cookie: user.cookie });
    assert.equal(result.status, 200);
    assert.deepEqual({ ...raw.prepare('SELECT day,used FROM ai_global').get() }, { day: '2026-10-22', used: 1 });
  } finally { Date.now = originalNow; }
});

test('stream completion also settles the original day after midnight', async () => {
  const { env, raw } = makeEnv();
  const user = await signup(env);
  const originalNow = Date.now;
  const before = Date.UTC(2026, 9, 22, 23, 59, 59);
  Date.now = () => before;
  env.AI = { async run() {
    Date.now = () => before + 2000;
    return new ReadableStream<Uint8Array>({ start(controller) {
      controller.enqueue(new TextEncoder().encode('data: {"usage":{"neurons":1}}\n\ndata: [DONE]\n\n'));
      controller.close();
    } });
  } } as unknown as Ai;
  try {
    const result = await call(env, 'POST', '/v1/ai/chat', { messages: [{ role: 'user', content: 'hello' }], consent: true, stream: true }, { cookie: user.cookie });
    assert.equal(result.status, 200);
    assert.deepEqual({ ...raw.prepare('SELECT day,used FROM ai_global').get() }, { day: '2026-10-22', used: 1 });
  } finally { Date.now = originalNow; }
});
