import { test } from 'node:test';
import assert from 'node:assert/strict';
import { checkPasswordPolicy, deriveAuthKey } from '../src/auth.ts';

test('derived key: 43-char base64url, deterministic, depends on e-mail and password', async () => {
  const k = await deriveAuthKey('a@example.com', 'correct horse battery', 1000);
  assert.match(k, /^[A-Za-z0-9_-]{43}$/);
  assert.equal(k, await deriveAuthKey('A@example.com ', 'correct horse battery', 1000)); // normalized e-mail
  assert.notEqual(k, await deriveAuthKey('b@example.com', 'correct horse battery', 1000));
  assert.notEqual(k, await deriveAuthKey('a@example.com', 'correct horse batterx', 1000));
});

test('the derived key never contains the password', async () => {
  const pw = 'correct horse battery';
  assert.ok(!(await deriveAuthKey('a@example.com', pw, 1000)).includes(pw));
});

test('password policy', () => {
  assert.equal(checkPasswordPolicy('short'), 'too_short');
  assert.equal(checkPasswordPolicy('password123'), 'too_common');
  assert.equal(checkPasswordPolicy('x'.repeat(201)), 'too_long');
  assert.equal(checkPasswordPolicy('correct horse battery'), 'ok');
});
