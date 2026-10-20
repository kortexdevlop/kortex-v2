import { test } from 'node:test';
import assert from 'node:assert/strict';
import { call, makeEnv, signup } from './helpers.ts';

const realFetch = globalThis.fetch;
function network(reply: (url: string, init: RequestInit) => Response) {
  const seen: { url: string; init: RequestInit }[] = [];
  globalThis.fetch = (async (input: RequestInfo | URL, init: RequestInit = {}) => { seen.push({ url: String(input), init }); return reply(String(input), init); }) as typeof fetch;
  return seen;
}
const restore = () => { globalThis.fetch = realFetch; };
const secrets = { OAUTH_GITHUB_ID: 'gh-id', OAUTH_GITHUB_SECRET: 'gh-secret', OAUTH_NOTION_ID: 'n-id', OAUTH_NOTION_SECRET: 'n-secret' };

test('only configured providers are offered, to signed-in users', async () => {
  const { env } = makeEnv(secrets);
  assert.equal((await call(env, 'GET', '/v1/oauth/providers')).status, 401);
  const { cookie } = await signup(env);
  const r = await call(env, 'GET', '/v1/oauth/providers', undefined, { cookie });
  assert.deepEqual(r.data.providers, ['github', 'notion']);
  assert.ok(!JSON.stringify(r.data).includes('secret'));
  const none = makeEnv();
  const c2 = await signup(none.env);
  assert.deepEqual((await call(none.env, 'GET', '/v1/oauth/providers', undefined, { cookie: c2.cookie })).data.providers, []);
});

test('a code is exchanged with the broker secret and only the useful token fields come back', async () => {
  const { env } = makeEnv(secrets);
  const { cookie } = await signup(env);
  const seen = network(() => new Response(JSON.stringify({ access_token: 'at', refresh_token: 'rt', expires_in: 3600, token_type: 'bearer', scope: 'repo', id_token: 'leak', user: { email: 'x' } })));
  try {
    const r = await call(env, 'POST', '/v1/oauth/exchange', {
      provider: 'github', code: 'abc', redirect_uri: 'https://workspace.kortex.site/oauth', code_verifier: 'v'.repeat(50),
    }, { cookie });
    assert.equal(r.status, 200);
    assert.deepEqual(Object.keys(r.data).sort(), ['access_token', 'expires_in', 'refresh_token', 'scope', 'token_type']);
    const sent = new URLSearchParams(String(seen[0]!.init.body));
    assert.equal(seen[0]!.url, 'https://github.com/login/oauth/access_token');
    assert.deepEqual([sent.get('client_id'), sent.get('client_secret'), sent.get('code'), sent.get('grant_type')], ['gh-id', 'gh-secret', 'abc', 'authorization_code']);
    assert.equal(sent.get('code_verifier')?.length, 50);
    // Providers that want Basic auth and JSON get exactly that.
    seen.length = 0;
    await call(env, 'POST', '/v1/oauth/exchange', { provider: 'notion', code: 'c', redirect_uri: 'http://127.0.0.1:5173/cb' }, { cookie });
    assert.equal(new Headers(seen[0]!.init.headers).get('authorization'), `Basic ${btoa('n-id:n-secret')}`);
    assert.equal(JSON.parse(String(seen[0]!.init.body)).client_secret, undefined);
  } finally { restore(); }
});

test('refresh works the same way and bad requests are refused before any network call', async () => {
  const { env } = makeEnv(secrets);
  const { cookie } = await signup(env);
  const seen = network(() => new Response(JSON.stringify({ access_token: 'new' })));
  try {
    const ok = await call(env, 'POST', '/v1/oauth/refresh', { provider: 'github', refresh_token: 'rt' }, { cookie });
    assert.equal(ok.data.access_token, 'new');
    assert.equal(new URLSearchParams(String(seen[0]!.init.body)).get('grant_type'), 'refresh_token');
    seen.length = 0;
    const bad: [unknown, number, string][] = [
      [{ provider: 'unknown', code: 'c', redirect_uri: 'https://workspace.kortex.site/x' }, 400, 'unknown_provider'],
      [{ provider: 'google', code: 'c', redirect_uri: 'https://workspace.kortex.site/x' }, 503, 'provider_unavailable'],
      [{ provider: 'github', code: 'c', redirect_uri: 'https://evil.example/cb' }, 400, 'redirect_not_allowed'],
      [{ provider: 'github', code: 'c', redirect_uri: 'http://example.org/cb' }, 400, 'redirect_not_allowed'],
      [{ provider: 'github', code: 'c', redirect_uri: 'https://workspace.kortex.site/x#frag' }, 400, 'redirect_not_allowed'],
      [{ provider: 'github', redirect_uri: 'https://workspace.kortex.site/x' }, 400, 'invalid_code'],
    ];
    for (const [body, status, code] of bad) {
      const r = await call(env, 'POST', '/v1/oauth/exchange', body, { cookie });
      assert.deepEqual([r.status, r.data.error], [status, code], JSON.stringify(body));
    }
    assert.equal(seen.length, 0);
  } finally { restore(); }
});

test('a provider refusal is reported with its short reason and nothing else', async () => {
  const { env } = makeEnv(secrets);
  const { cookie } = await signup(env);
  network(() => new Response(JSON.stringify({ error: 'invalid_grant', error_description: 'secret details', client_secret: 'x' }), { status: 400 }));
  try {
    const r = await call(env, 'POST', '/v1/oauth/exchange', { provider: 'github', code: 'bad', redirect_uri: 'https://workspace.kortex.site/x' }, { cookie });
    assert.deepEqual([r.status, r.data], [502, { error: 'oauth_failed', provider_error: 'invalid_grant' }]);
    network(() => { throw new Error('down'); });
    assert.equal((await call(env, 'POST', '/v1/oauth/exchange', { provider: 'github', code: 'c', redirect_uri: 'https://workspace.kortex.site/x' }, { cookie })).data.error, 'oauth_unreachable');
  } finally { restore(); }
});
