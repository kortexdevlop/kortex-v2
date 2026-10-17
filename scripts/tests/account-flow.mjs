import {deriveAuthKey} from '../../services/shared/src/auth.ts';
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';

// This creates and deletes only its own uniquely named QA account, never existing accounts.
export async function accountFlow(page, origin, apiOrigin) {
  let registered = false, erased = false;
  const email = `deployment-${randomUUID()}@example.invalid`;
  const password = `Kortex-QA-${randomUUID()}!`;
  const requests = [];
  page.on('request', request => {
    if (request.method() === 'POST' && /\/v1\/auth\/(register|login)$/.test(new URL(request.url()).pathname)) requests.push(request.postDataJSON());
  });
  try {
  await page.goto(`${origin}/compte`, {waitUntil:'networkidle'});
  await page.locator('[data-view=auth]').waitFor({state:'visible'});
  await page.locator('[data-switch]').click();
  await page.locator('[data-form] [name=email]').fill(email);
  await page.locator('[data-form] [name=password]').fill(password);
  const registration = page.waitForResponse(response => response.url().endsWith('/v1/auth/register'));
  await page.locator('[data-submit]').click();
  const created = await registration;
  assert.equal(created.status(),201);
  registered = true;
  await page.locator('[data-view=me]').waitFor({state:'visible'});
  assert.equal(await page.locator('[data-email]').textContent(),email);
  const cookies = await page.context().cookies();
  const session = cookies.find(cookie => cookie.name === '__Host-kortex_session');
  assert.ok(session?.secure && session.httpOnly);
  assert.equal(session.domain,new URL(apiOrigin).hostname);
  await page.reload({waitUntil:'networkidle'});
  await page.locator('[data-view=me]').waitFor({state:'visible'});
  const setting = page.waitForResponse(response => response.url().endsWith('/v1/settings') && response.request().method() === 'PUT');
  await page.locator('[data-confirm-install]').uncheck();
  assert.equal((await setting).status(),200);
  const downloaded = page.waitForEvent('download');
  await page.locator('[data-export]').click();
  const download = await downloaded;
  const stream = await download.createReadStream();
  let text = '';
  for await (const chunk of stream) text += chunk.toString();
  const exported = JSON.parse(text);
  assert.equal(exported.user.email,email);
  assert.ok(!text.includes('password_hash'));
  await page.locator('[data-logout]').click();
  await page.locator('[data-view=auth]').waitFor({state:'visible'});
  await page.locator('[data-form] [name=email]').fill(email);
  await page.locator('[data-form] [name=password]').fill(password + '-wrong');
  await page.locator('[data-submit]').click();
  await page.getByText('E-mail ou mot de passe incorrect.',{exact:true}).waitFor();
  await page.locator('[data-form] [name=password]').fill(password);
  await page.locator('[data-submit]').click();
  await page.locator('[data-view=me]').waitFor({state:'visible'});
  await page.locator('[data-del-open]').click();
  await page.locator('[data-del-form] [name=password]').fill(password);
  const deleted = page.waitForResponse(response => response.request().method() === 'DELETE' && response.url().endsWith('/v1/me'));
  await page.locator('[data-del-form] button[type=submit]').click();
  assert.equal((await deleted).status(),200);
  erased = true;
  await page.locator('[data-view=auth]').waitFor({state:'visible'});
  assert.ok(requests.length >= 3);
  for (const body of requests) {
    assert.equal(typeof body.auth_key,'string');
    assert.ok(!JSON.stringify(body).includes(password));
    assert.ok(!('password' in body));
  }
  console.log('Account OK: register, secure host-only cookie, reload, settings, export, logout, wrong/right password, delete QA account.');
  } finally {
    if (registered && !erased) {
      const auth_key = await deriveAuthKey(email,password);
      const options = {headers:{Origin:origin}, data:{email,auth_key,kdf:1}};
      const login = await page.request.post(`${apiOrigin}/v1/auth/login`,options);
      assert.ok(login.ok(),'QA cleanup login failed');
      const deleted = await page.request.delete(`${apiOrigin}/v1/me`,{headers:{Origin:origin},data:{auth_key,kdf:1}});
      assert.ok(deleted.ok(),'QA account cleanup failed');
    }
  }

}
