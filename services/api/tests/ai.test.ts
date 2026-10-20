import { test } from 'node:test';
import assert from 'node:assert/strict';
import { creditsView, limits } from '../src/credits.ts';
import { MODELS, neurons } from '../src/ai-models.ts';
import { call, callRaw, makeEnv, signup } from './helpers.ts';

type Usage = { prompt_tokens?: number; completion_tokens?: number; neurons?: number };
type ToolCall = { id: string; type: 'function'; function: { name: string; arguments: string } };
const GEMMA = MODELS.gemma!;

/** Provider look-alike: OpenAI-style chat completion, or an SSE stream when asked to stream. */
function fakeAi(opts: { usage?: Usage; text?: string; toolCalls?: ToolCall[]; fail?: string; delayMs?: number; stream?: string[] } = {}) {
  const calls: { model: string; input: Record<string, unknown> }[] = [];
  const ai = {
    async run(model: string, input: Record<string, unknown>) {
      calls.push({ model, input });
      if (opts.delayMs) await new Promise((r) => setTimeout(r, opts.delayMs));
      if (opts.fail) throw new Error(opts.fail);
      if (input.stream === true) return sseStream(opts.stream ?? [], opts.usage);
      const message = { role: 'assistant', content: opts.toolCalls ? null : (opts.text ?? 'bonjour'), ...(opts.toolCalls ? { tool_calls: opts.toolCalls } : {}) };
      return { choices: [{ index: 0, message, finish_reason: opts.toolCalls ? 'tool_calls' : 'stop' }], usage: opts.usage };
    },
  };
  return { ai: ai as unknown as Ai, calls };
}

/** Provider SSE as seen on a real Gemma 4 call: delta chunks, then a usage chunk, then [DONE]. */
function sseStream(lines: string[], usage?: Usage, failAfter?: number): ReadableStream<Uint8Array> {
  const enc = new TextEncoder();
  const chunks = lines.map((l) => `data: ${l}\n\n`);
  chunks.push(`data: ${JSON.stringify({ response: '', usage })}\n\n`, 'data: [DONE]\n\n');
  let i = 0;
  return new ReadableStream({
    async pull(controller) {
      if (failAfter !== undefined && i >= failAfter) { controller.error(new Error('provider reset')); return; }
      if (i >= chunks.length) { controller.close(); return; }
      controller.enqueue(enc.encode(chunks[i++]));
    },
  });
}
const delta = (d: Record<string, unknown>, finish: string | null = null) => JSON.stringify({ choices: [{ delta: d, finish_reason: finish, index: 0 }], id: 'internal', p: 'noise' });

const chatBody = (extra: Record<string, unknown> = {}) => ({ messages: [{ role: 'user', content: 'Dis bonjour' }], consent: true, max_tokens: 100, ...extra });
const cleanerTool = { type: 'function', function: { name: 'text__cleaner', description: 'Nettoie un texte', parameters: { type: 'object', properties: { text: { type: 'string' } }, required: ['text'] } } };

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
  assert.equal((await post({ messages: [{ role: 'user', content: 'x'.repeat(41_000) }], consent: true })).data.error, 'invalid_messages');
  // A long source text fits in one message; the whole request stays capped at 48 000 characters.
  assert.equal((await post({ messages: [{ role: 'user', content: 'x'.repeat(30_000) }], consent: true })).status, 200);
  assert.equal((await post({ messages: [{ role: 'user', content: 'x'.repeat(30_000) }, { role: 'assistant', content: 'y'.repeat(20_000) }], consent: true })).data.error, 'messages_too_large');
  assert.equal((await post(chatBody({ tools: [] }))).data.error, 'invalid_tools');
  assert.equal((await post(chatBody({ tools: [{ type: 'function', function: { name: 'bad name!', description: 'x', parameters: {} } }] }))).data.error, 'invalid_tools');
  assert.equal((await post(chatBody({ tool_choice: 'auto' }))).data.error, 'invalid_tool_choice', 'tool_choice needs tools');
  assert.equal((await post(chatBody({ tools: [cleanerTool], tool_choice: 'sometimes' }))).data.error, 'invalid_tool_choice');
  assert.equal((await post(chatBody({ stream: 'yes' }))).data.error, 'invalid_stream');
  assert.equal((await post({ messages: [{ role: 'tool', content: 'x' }], consent: true })).data.error, 'invalid_messages', 'tool result without its call id');
  assert.equal((await post({ messages: [{ role: 'assistant', content: null }], consent: true })).data.error, 'invalid_messages', 'empty assistant turn');
  assert.equal((await call(env, 'POST', '/v1/ai/chat', chatBody())).status, 401);
});

test('a successful call charges the REAL usage, not the reservation', async () => {
  const { ai, calls } = fakeAi({ usage: { prompt_tokens: 20, completion_tokens: 50 }, text: 'salut' });
  const { env, raw } = makeEnv({ AI: ai });
  const { cookie } = await signup(env);
  const r = await call(env, 'POST', '/v1/ai/chat', chatBody(), { cookie });
  assert.equal(r.status, 200);
  assert.equal(r.data.message.content, 'salut');
  assert.equal(r.data.finish_reason, 'stop');
  assert.equal(calls.length, 1);
  const cost = neurons(GEMMA, 20, 50);
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
  const cost = neurons(GEMMA, 4, 100); // ~3.07 neurons for this body
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
  assert.ok(glob.used > neurons(GEMMA, 100, 2000) * 1.9, 'both users drew from the same day budget');
});

test('without an AI binding the endpoint reports ai_unavailable (503) and charges nothing', async () => {
  const { env } = makeEnv();
  const { cookie } = await signup(env);
  const r = await call(env, 'POST', '/v1/ai/chat', chatBody(), { cookie });
  assert.deepEqual([r.status, r.data.error], [503, 'ai_unavailable']);
});

test('the model is called with thinking off, a bounded output and the forwarded tools', async () => {
  const { ai, calls } = fakeAi({ usage: { prompt_tokens: 30, completion_tokens: 10, neurons: 0.5 } });
  const { env } = makeEnv({ AI: ai });
  const { cookie } = await signup(env);
  await call(env, 'POST', '/v1/ai/chat', chatBody({ tools: [cleanerTool], tool_choice: 'auto', max_tokens: 321 }), { cookie });
  const sent = calls[0]!;
  assert.equal(sent.model, GEMMA.id);
  assert.deepEqual(sent.input.chat_template_kwargs, { enable_thinking: false });
  assert.equal(sent.input.max_completion_tokens, 321);
  assert.equal(sent.input.tool_choice, 'auto');
  assert.equal((sent.input.tools as unknown[]).length, 1);
  assert.equal(sent.input.stream, undefined);
});

test('tool calls come back structured, and tool results can be sent on the next round', async () => {
  const toolCalls: ToolCall[] = [{ id: 'call_1', type: 'function', function: { name: 'text__cleaner', arguments: '{"text":"@texte1"}' } }];
  const { ai, calls } = fakeAi({ toolCalls, usage: { prompt_tokens: 40, completion_tokens: 12, neurons: 0.7 } });
  const { env } = makeEnv({ AI: ai });
  const { cookie } = await signup(env);
  const r = await call(env, 'POST', '/v1/ai/chat', chatBody({ tools: [cleanerTool] }), { cookie });
  assert.equal(r.status, 200);
  assert.equal(r.data.finish_reason, 'tool_calls');
  assert.equal(r.data.message.content, null);
  assert.deepEqual(r.data.message.tool_calls, toolCalls);
  const second = await call(env, 'POST', '/v1/ai/chat', {
    consent: true,
    tools: [cleanerTool],
    messages: [
      { role: 'user', content: 'Nettoie @texte1' },
      { role: 'assistant', content: null, tool_calls: toolCalls },
      { role: 'tool', tool_call_id: 'call_1', content: '{"text":"@call_1.text"}' },
    ],
  }, { cookie });
  assert.equal(second.status, 200);
  assert.deepEqual((calls[1]!.input.messages as { role: string }[]).map((m) => m.role), ['user', 'assistant', 'tool']);
});

test('the provider\'s own neuron count is what gets charged, with a token-based fallback', async () => {
  const { env, raw } = makeEnv({ AI: fakeAi({ usage: { prompt_tokens: 1000, completion_tokens: 1000, neurons: 1.25 } }).ai });
  const { cookie } = await signup(env);
  await call(env, 'POST', '/v1/ai/chat', chatBody(), { cookie });
  assert.ok(Math.abs((raw.prepare('SELECT used FROM ai_global').get() as { used: number }).used - 1.25) < 1e-9, 'uses usage.neurons, not the token prices');
  const { env: env2, raw: raw2 } = makeEnv({ AI: fakeAi({ usage: { prompt_tokens: 1000, completion_tokens: 1000 } }).ai });
  const u = await signup(env2);
  await call(env2, 'POST', '/v1/ai/chat', chatBody(), { cookie: u.cookie });
  assert.ok(Math.abs((raw2.prepare('SELECT used FROM ai_global').get() as { used: number }).used - neurons(GEMMA, 1000, 1000)) < 1e-6);
});

test('streaming re-emits only deltas, ends with the credits and settles the provider neurons', async () => {
  const lines = [
    delta({ content: '', role: 'assistant' }),
    delta({ content: 'Les' }),
    delta({ content: ' chats' }),
    delta({}, 'stop'),
  ];
  const { ai, calls } = fakeAi({ stream: lines, usage: { prompt_tokens: 21, completion_tokens: 41, neurons: 1.3 } });
  const { env, raw } = makeEnv({ AI: ai });
  const { cookie } = await signup(env);
  const res = await callRaw(env, 'POST', '/v1/ai/chat', chatBody({ stream: true }), { cookie });
  assert.equal(res.status, 200);
  assert.match(res.headers.get('content-type') ?? '', /text\/event-stream/);
  const text = await res.text();
  assert.equal(calls[0]!.input.stream, true);
  assert.deepEqual((calls[0]!.input as { stream_options: unknown }).stream_options, { include_usage: true });
  const events = text.split('\n\n').filter(Boolean);
  const deltas = events.filter((e) => e.startsWith('data:')).map((e) => JSON.parse(e.slice(5)));
  assert.deepEqual(deltas.map((d) => d.delta.content ?? null), ['Les', ' chats', null]);
  assert.equal(deltas.at(-1).finish_reason, 'stop');
  assert.doesNotMatch(text, /internal|noise|neurons/, 'provider internals never reach the client');
  const done = events.find((e) => e.startsWith('event: done'))!;
  assert.equal(JSON.parse(done.split('data:')[1]!).credits.status, 'ok');
  assert.ok(Math.abs((raw.prepare('SELECT used FROM ai_global').get() as { used: number }).used - 1.3) < 1e-9);
  assert.equal((raw.prepare('SELECT requests FROM ai_ledger').get() as { requests: number }).requests, 1);
});

test('streamed tool calls pass through as fragments', async () => {
  const lines = [
    delta({ role: 'assistant', content: '' }),
    delta({ tool_calls: [{ index: 0, id: 'call_9', type: 'function', function: { name: 'text__cleaner', arguments: '' } }] }),
    delta({ tool_calls: [{ index: 0, id: null, function: { name: null, arguments: '{"text":"@texte1"}' } }] }),
    delta({}, 'tool_calls'),
  ];
  const { env } = makeEnv({ AI: fakeAi({ stream: lines, usage: { neurons: 0.8 } }).ai });
  const { cookie } = await signup(env);
  const text = await (await callRaw(env, 'POST', '/v1/ai/chat', chatBody({ stream: true, tools: [cleanerTool] }), { cookie })).text();
  const deltas = text.split('\n\n').filter((e) => e.startsWith('data:')).map((e) => JSON.parse(e.slice(5)));
  const fragments = deltas.flatMap((d) => d.delta.tool_calls ?? []);
  assert.equal(fragments[0].function.name, 'text__cleaner');
  assert.equal(fragments.map((f) => f.function.arguments).join(''), '{"text":"@texte1"}');
  assert.equal(deltas.at(-1).finish_reason, 'tool_calls');
});

test('a client that leaves a stream early is charged an estimate, once', async () => {
  const lines = Array.from({ length: 50 }, (_, i) => delta({ content: `mot${i} ` }));
  const { env, raw } = makeEnv({ AI: fakeAi({ stream: lines, usage: { neurons: 9 } }).ai });
  const { cookie } = await signup(env);
  const res = await callRaw(env, 'POST', '/v1/ai/chat', chatBody({ stream: true, max_tokens: 1000 }), { cookie });
  const reader = res.body!.getReader();
  await reader.read();
  await reader.cancel();
  await new Promise((r) => setTimeout(r, 20));
  const used = (raw.prepare('SELECT used FROM ai_global').get() as { used: number }).used;
  const reservedFully = neurons(GEMMA, 6, 1000);
  assert.ok(used > 0 && used < reservedFully / 10, `settled on an estimate (${used}), not the full reservation (${reservedFully})`);
  assert.equal((raw.prepare('SELECT requests FROM ai_ledger').get() as { requests: number }).requests, 1, 'settled exactly once');
});

test('a provider failure in the middle of a stream ends with an error event and no double charge', async () => {
  const enc = new TextEncoder();
  const ai = { async run() {
    let sent = false;
    return new ReadableStream<Uint8Array>({ pull(controller) {
      if (!sent) { sent = true; controller.enqueue(enc.encode(`data: ${delta({ content: 'début' })}\n\n`)); return; }
      controller.error(new Error('provider reset'));
    } });
  } } as unknown as Ai;
  const { env, raw } = makeEnv({ AI: ai });
  const { cookie } = await signup(env);
  const text = await (await callRaw(env, 'POST', '/v1/ai/chat', chatBody({ stream: true }), { cookie })).text();
  assert.match(text, /event: error\ndata: \{"error":"ai_unavailable"\}/);
  assert.equal((raw.prepare('SELECT requests FROM ai_ledger').get() as { requests: number }).requests, 1);
});
