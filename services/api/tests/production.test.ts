import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readJson} from '../src/http.ts';
import {cleanup} from '../src/index.ts';
import {rateLimit} from '../src/ratelimit.ts';
import {makeEnv} from './helpers.ts';

test('JSON byte cap also stops chunked multibyte input without content-length', async () => {
  let cancelled = false;
  const stream = new ReadableStream<Uint8Array>({
    start(controller) { controller.enqueue(new TextEncoder().encode('"ééé"')); },
    cancel() { cancelled = true; },
  });
  const req = new Request('https://api.kortex.site', {method:'POST', body:stream, duplex:'half'} as RequestInit);
  await assert.rejects(readJson(req, 5), (error: any) => error.status === 413);
  assert.equal(cancelled, true);
});

test('security keys are hashed and the expiry sweep preserves current data', async () => {
  const {env, raw} = makeEnv();
  const now = Date.UTC(2026, 9, 16);
  await rateLimit({env} as any, 'login:email:private@example.com', 5, 60, now);
  const key = (raw.prepare('SELECT key FROM rate_limits').get() as {key:string}).key;
  assert.match(key, /^[a-f0-9]{64}$/);
  raw.prepare('INSERT INTO rate_limits VALUES (?, ?, ?)').run('old', Math.floor(now / 1000) - 90000, 1);
  raw.prepare('INSERT INTO users VALUES (?, ?, ?, ?)').run('u', 'test@example.com', 'unused', now);
  for (const [id, expiry] of [['expired', now - 1], ['active', now + 10000]] as const) raw.prepare('INSERT INTO sessions VALUES (?, ?, ?, ?, ?)').run(id, 'u', now, expiry, null);
  await cleanup(env, now);
  assert.deepEqual(raw.prepare('SELECT id FROM sessions').all().map((r:any) => r.id), ['active']);
  assert.deepEqual(raw.prepare('SELECT key FROM rate_limits').all().map((r:any) => r.key), [key]);
});
