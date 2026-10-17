import { test } from 'node:test';
import assert from 'node:assert/strict';
import { creditsView, limits } from '../src/credits.ts';
import { MODELS, neurons } from '../src/ai-models.ts';
import { call, makeEnv, signup } from './helpers.ts';
import type { Env } from '../src/env.ts';

type Usage = { prompt_tokens?: number; completion_tokens?: number };
function fakeAi(opts: { usage?: Usage; text?: string; fail?: string; delayMs?: number } = {}) {
  const calls: unknown[] = [];
  const ai = {
    async run(model: string, input: unknown) {
      calls.push({ model, input });
      if (opts.delayMs) await new Promise((r) => setTimeout(r, opts.delayMs));
      if (opts.fail) throw new Error(opts.fail);
      return { response: opts.text ?? 'bonjour', usage: opts.usage };
    },
  };
  return { ai: ai as unknown as Ai, calls };
}

const chatBody = (extra: Record<string, unknown> = {}) => ({ messages: [{ role: 'user', content: 'Dis bonjour' }], consent: true, max_tokens: 100, ...extra });

test('credits start at 100% and the view exposes percentages only', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  const r = await call(env, 'GET', '/v1/ai/credits', undefined, { cookie });
  assert.deepEqual(r.data, { percent: 100, status: 'ok', resets_in_seconds: null, available: false });
});

test('chat requires consent, valid messages, a known model and a sane max_tokens', async () => {
  const { env } = makeEnv({ AI: fakeAi().ai });
  const { cookie } = await signup(env);
  const post = (b: unknown) => call(env, 'POST', '/v1/ai/chat', b, { cookie });
  assert.equal((await post(chatBody({ consent: false }))).data.error, 'consent_required');
  assert.equal((await post({ messages: [], consent: true })).data.error, 'invalid_messages');
  assert.equal((await post({ messages: [{ role: 'root', content: 'x' }], consent: true })).data.error, 'invalid_messages');
  assert.equal((await post(chatBody({ model: 'gpt-9' }))).data.error, 'invalid_model');
  assert.equal((await post(chatBody({ max_tokens: 99999 }))).data.error, 'invalid_max_tokens');
  assert.equal((await post({ messages: [{ role: 'user', content: 'x'.repeat(9000) }], consent: true })).data.error, 'invalid_messages');
  assert.equal((await call(env, 'POST', '/v1/ai/chat', chatBody())).status, 401);
});

test('a successful call charges the REAL usage, not the reservation', async () => {
  const { ai, calls } = fakeAi({ usage: { prompt_tokens: 20, completion_tokens: 50 }, text: 'salut' });
  const { env, raw } = makeEnv({ AI: ai });
  const { cookie } = await signup(env);
  const r = await call(env, 'POST', '/v1/ai/chat', chatBody(), { cookie });
  assert.equal(r.status, 200);
  assert.equal(r.data.content, 'salut');
  assert.equal(calls.length, 1);
  const cost = neurons(MODELS.fast!, 20, 50);
  const bucket = raw.prepare('SELECT tokens FROM ai_buckets').get() as { tokens: number };
  assert.ok(Math.abs(bucket.tokens - (limits(env).userCap - cost)) < 0.001, `bucket ${bucket.tokens}`);
  const glob = raw.prepare('SELECT used FROM ai_global').get() as { used: number };
  assert.ok(Math.abs(glob.used - cost) < 0.001, `global ${glob.used}`);
  const ledger = raw.prepare('SELECT requests, neurons FROM ai_ledger').get() as { requests: number; neurons: number };
  assert.equal(ledger.requests, 1);
  assert.ok(Math.abs(ledger.neurons - cost) < 0.001);
});

test('the ledger and tables never store prompts or answers', async () => {
  const { env, raw } = makeEnv({ AI: fakeAi({ text: 'REPONSE-SECRETE' }).ai });
  const { cookie } = await signup(env);
  await call(env, 'POST', '/v1/ai/chat', { messages: [{ role: 'user', content: 'PROMPT-SECRET' }], consent: true, max_tokens: 50 }, { cookie });
  for (const t of ['ai_buckets', 'ai_global', 'ai_ledger', 'usage_daily']) {
    assert.doesNotMatch(JSON.stringify(raw.prepare(`SELECT * FROM ${t}`).all()), /SECRET/, t);
  }
});

test('credits drop, then refill with time (400 neurons per 3 h by default)', async () => {
  const { env, raw } = makeEnv({ AI: fakeAi({ usage: { prompt_tokens: 100, completion_tokens: 10_000 } }).ai });
  const { cookie } = await signup(env);
  await call(env, 'POST', '/v1/ai/chat', chatBody({ max_tokens: 1024 }), { cookie });
  const uid = (raw.prepare('SELECT id FROM users').get() as { id: string }).id;
  const low = await creditsView(env, uid);
  assert.ok(low.percent < 100, `percent ${low.percent}`);
  // Pretend the last update happened 3 hours ago: +400 neurons (a third of the cap).
  raw.exec('UPDATE ai_buckets SET updated_at = updated_at - 3 * 3600 * 1000');
  const later = await creditsView(env, uid);
  assert.ok(later.percent >= Math.min(100, low.percent + 33), `refilled ${low.percent} -> ${later.percent}`);
  raw.exec('UPDATE ai_buckets SET updated_at = updated_at - 99 * 3600 * 1000');
  assert.equal((await creditsView(env, uid)).percent, 100, 'refill is capped');
});

test('an empty bucket answers 429 user_empty with Retry-After, and nothing is spent', async () => {
  const { ai, calls } = fakeAi();
  const { env, raw } = makeEnv({ AI: ai, AI_USER_CAP: '2' });
  const { cookie } = await signup(env);
  const r = await call(env, 'POST', '/v1/ai/chat', chatBody({ max_tokens: 1024 }), { cookie });
  assert.equal(r.status, 429);
  assert.equal(r.data.error, 'user_empty');
  assert.ok(Number(r.headers.get('retry-after')) > 0);
  assert.equal(calls.length, 0, 'the model must not be called');
  assert.equal((raw.prepare('SELECT COUNT(*) AS n FROM ai_global').get() as { n: number }).n, 0);
});

test('global daily budget exhausted: everyone is paused until UTC midnight', async () => {
  const { env } = makeEnv({ AI: fakeAi().ai, AI_GLOBAL_DAILY: '0.5' });
  const { cookie } = await signup(env);
  const r = await call(env, 'POST', '/v1/ai/chat', chatBody({ max_tokens: 1024 }), { cookie });
  assert.equal(r.status, 429);
  assert.equal(r.data.error, 'ai_paused');
  assert.ok(Number(r.headers.get('retry-after')) > 0 && Number(r.headers.get('retry-after')) <= 86_400);
  const { env: env2, raw } = makeEnv({ AI: fakeAi().ai, AI_GLOBAL_DAILY: '10' });
  const u = await signup(env2);
  raw.exec(`INSERT INTO ai_global (day, used) VALUES (date('now'), 10)`);
  assert.equal((await call(env2, 'GET', '/v1/ai/credits', undefined, { cookie: u.cookie })).data.status, 'paused');
});

test('a failing model costs nothing; a provider quota error pauses the day', async () => {
  const { env, raw } = makeEnv({ AI: fakeAi({ fail: 'upstream boom' }).ai });
  const { cookie } = await signup(env);
  const r = await call(env, 'POST', '/v1/ai/chat', chatBody(), { cookie });
  assert.equal(r.status, 502);
  const bucket = raw.prepare('SELECT tokens FROM ai_buckets').get() as { tokens: number };
  assert.ok(Math.abs(bucket.tokens - limits(env).userCap) < 0.001, 'full refund');
  const { env: env2 } = makeEnv({ AI: fakeAi({ fail: 'you have used your daily free allocation of 10,000 neurons' }).ai });
  const u = await signup(env2);
  const q = await call(env2, 'POST', '/v1/ai/chat', chatBody(), { cookie: u.cookie });
  assert.deepEqual([q.status, q.data.error], [429, 'ai_paused']);
  assert.equal((await call(env2, 'GET', '/v1/ai/credits', undefined, { cookie: u.cookie })).data.status, 'paused');
});

test('parallel requests cannot overspend the bucket', async () => {
  const cost = neurons(MODELS.fast!, 4, 100); // ~3.07 neurons for this body
  const { env } = makeEnv({ AI: fakeAi({ delayMs: 40, usage: { prompt_tokens: 4, completion_tokens: 100 } }).ai, AI_USER_CAP: String(cost * 2.5) });
  const { cookie } = await signup(env);
  const results = await Promise.all(Array.from({ length: 6 }, () => call(env, 'POST', '/v1/ai/chat', chatBody(), { cookie })));
  const ok = results.filter((r) => r.status === 200).length;
  assert.equal(ok, 2, `exactly two fit, got ${ok}: ${results.map((r) => r.status)}`);
  assert.ok(results.filter((r) => r.status === 429).length >= 3);
});

test('buckets are per user while the global budget is shared', async () => {
  const { env, raw } = makeEnv({ AI: fakeAi({ usage: { prompt_tokens: 100, completion_tokens: 2000 } }).ai });
  const a = await signup(env, 'a@example.com');
  const b = await signup(env, 'b@example.com');
  await call(env, 'POST', '/v1/ai/chat', chatBody({ max_tokens: 1024 }), { cookie: a.cookie });
  assert.equal((await call(env, 'GET', '/v1/ai/credits', undefined, { cookie: b.cookie })).data.percent, 100);
  await call(env, 'POST', '/v1/ai/chat', chatBody({ max_tokens: 1024 }), { cookie: b.cookie });
  const glob = raw.prepare('SELECT used FROM ai_global').get() as { used: number };
  assert.ok(glob.used > neurons(MODELS.fast!, 100, 2000) * 1.9, 'both users drew from the same day budget');
});

test('without an AI binding the endpoint reports ai_unavailable (503) and charges nothing', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  const r = await call(env, 'POST', '/v1/ai/chat', chatBody(), { cookie });
  assert.deepEqual([r.status, r.data.error], [503, 'ai_unavailable']);
});
