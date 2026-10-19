import { test } from 'node:test';
import assert from 'node:assert/strict';
import { aiStats, appOf, creditsView, limits, refillFactor, reserve, settle, utcDay } from '../src/credits.ts';
import { call, makeEnv, signup } from './helpers.ts';

/** Provider look-alike: a fixed usage per call. */
const fakeAi = (neurons = 50) => ({ async run() { return { choices: [{ index: 0, message: { role: 'assistant', content: 'ok' }, finish_reason: 'stop' }], usage: { prompt_tokens: 10, completion_tokens: 10, neurons } }; } }) as unknown as Ai;
const body = { messages: [{ role: 'user', content: 'Bonjour' }], consent: true, max_tokens: 20 };
const userId = (raw: ReturnType<typeof makeEnv>['raw']): string => (raw.prepare('SELECT id FROM users').get() as { id: string }).id;

test('the refill of every account slows down as the shared daily budget runs out', async () => {
  assert.deepEqual([0, 0.69, 0.7, 0.89, 0.9, 1].map((f) => refillFactor(f * 9000, 9000)), [1, 1, 0.5, 0.5, 0.25, 0.25]);
  const { env, raw } = makeEnv();
  await signup(env);
  const uid = userId(raw);
  const now = Date.now();
  // The account is at 400 neurons, last updated three hours ago: a full refill step is due.
  raw.prepare('INSERT INTO ai_buckets (user_id, tokens, updated_at) VALUES (?1, 400, ?2)').run(uid, now - 3 * 3600_000);
  const normal = limits(env).userCap;
  const full = await creditsView(env, uid, now);
  assert.equal(full.percent, Math.round(((400 + limits(env).refillPer3h) / normal) * 100), 'normal day: +400');
  // Late in the day (80 % of the budget spent) the same wait gives half as much.
  raw.prepare('INSERT INTO ai_global (day, used) VALUES (?1, ?2)').run(utcDay(now), 7_400);
  const slow = await creditsView(env, uid, now);
  assert.equal(slow.percent, Math.round(((400 + limits(env).refillPer3h * 0.5) / normal) * 100), 'a tight day: +200');
  assert.ok(slow.percent < full.percent);
});

test('no single account can take more than its share of the day, and nothing is lost when it is refused', async () => {
  const { env, raw } = makeEnv({ AI_USER_DAILY: '120' });
  await signup(env, 'heavy@example.com');
  const heavy = userId(raw);
  const now = Date.now();
  const first = await reserve(env, heavy, 100, now);
  assert.equal(first.ok, true);
  const refused = await reserve(env, heavy, 30, now);
  assert.deepEqual(refused.ok ? null : refused.reason, 'user_daily');
  assert.ok(!refused.ok && refused.retryAfter > 0 && refused.retryAfter <= 86_400);
  const global = raw.prepare('SELECT used FROM ai_global').get() as { used: number };
  assert.equal(global.used, 100, 'a refused request reserved nothing in the shared budget');
  // The view says the day is spent only once the share is used up; the bucket still has credit.
  await reserve(env, heavy, 19, now);
  const view = await creditsView(env, heavy, now);
  assert.equal(view.status, 'empty');
  assert.ok((view.resets_in_seconds ?? 0) > 0);
  // Another account is not affected by the first one.
  await signup(env, 'light@example.com');
  const other = (raw.prepare('SELECT id FROM users WHERE email != ?1').get('heavy@example.com') as { id: string }).id;
  assert.equal((await reserve(env, other, 100, now)).ok, true);
});

test('settling returns what the call did not use to the account share and records the app', async () => {
  const { env, raw } = makeEnv({ AI_USER_DAILY: '500' });
  await signup(env);
  const uid = userId(raw);
  const now = Date.now();
  await reserve(env, uid, 200, now);
  await settle(env, uid, 200, 60, 'gemma', now, 'flow');
  await reserve(env, uid, 50, now);
  await settle(env, uid, 50, 0, 'gemma', now, 'mind'); // failed call: full refund, no usage row
  const share = raw.prepare('SELECT neurons, requests FROM ai_user_day').get() as { neurons: number; requests: number };
  assert.deepEqual([Math.round(share.neurons), share.requests], [60, 1]);
  const apps = raw.prepare('SELECT app, requests, neurons FROM ai_app ORDER BY app').all() as { app: string; requests: number; neurons: number }[];
  assert.deepEqual(apps.map((a) => [a.app, a.requests, Math.round(a.neurons)]), [['flow', 1, 60]]);
});

test('the app is read from the request origin and the gateway records it', async () => {
  const req = (origin: string | null) => new Request('https://api.kortex.site/v1/ai/chat', { method: 'POST', headers: origin ? { origin } : {} });
  assert.deepEqual(['https://flow.kortex.site', 'https://agent.kortex.site', 'https://search.kortex.site', 'https://kortex.site', null, 'not a url'].map((o) => appOf(req(o))), ['flow', 'mind', 'seek', 'site', 'desktop', 'site']);
  const { env, raw } = makeEnv({ AI: fakeAi(50), ALLOWED_ORIGINS: 'https://flow.kortex.site,https://agent.kortex.site,https://kortex.site' });
  const { cookie } = await signup(env, 'a@example.com');
  assert.equal((await call(env, 'POST', '/v1/ai/chat', body, { cookie, origin: 'https://flow.kortex.site' })).status, 200);
  assert.equal((await call(env, 'POST', '/v1/ai/chat', body, { cookie, origin: 'https://agent.kortex.site' })).status, 200);
  assert.equal((await call(env, 'POST', '/v1/ai/chat', body, { cookie, origin: 'https://agent.kortex.site' })).status, 200);
  const apps = raw.prepare('SELECT app, requests FROM ai_app ORDER BY app').all() as { app: string; requests: number }[];
  assert.deepEqual(apps.map((a) => [a.app, a.requests]), [['flow', 1], ['mind', 2]]);
});

test('the owner statistics show totals, apps and a hashed top without any raw account id', async () => {
  const { env, raw } = makeEnv({ AI: fakeAi(80), ALLOWED_ORIGINS: 'https://flow.kortex.site,https://kortex.site' });
  const { cookie } = await signup(env, 'stats@example.com');
  for (let i = 0; i < 3; i++) await call(env, 'POST', '/v1/ai/chat', body, { cookie, origin: 'https://flow.kortex.site' });
  const stats = await aiStats(env);
  assert.equal(stats.global.cap, 9000);
  assert.equal(stats.global.used, 240);
  assert.equal(stats.global.percent, 3);
  assert.equal(stats.global.factor, 1);
  assert.deepEqual([stats.today.requests, stats.today.users], [3, 1]);
  assert.deepEqual(stats.apps.map((a) => [a.app, a.requests, a.neurons]), [['flow', 3, 240]]);
  assert.equal(stats.days.at(-1)?.requests, 3);
  const text = JSON.stringify(stats);
  assert.ok(!text.includes(userId(raw)) && !text.includes('stats@example.com'), 'no raw id or e-mail');
  assert.match(stats.today.top[0]!.user, /^[0-9a-f]{8}$/);
});
