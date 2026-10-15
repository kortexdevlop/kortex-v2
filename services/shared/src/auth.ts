// Client-side password handling (browser, desktop and CLI share this contract; see ADR 0005).
// The password never leaves the device: only a key derived with a slow KDF is sent, so the
// Worker (10 ms CPU on the free plan) only needs a cheap hash of that already-strong key.

export const KDF_VERSION = 1;
export const KDF_ITERATIONS = 600_000;

const enc = new TextEncoder();
const b64url = (bytes: Uint8Array): string => {
  let s = '';
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};

/** PBKDF2-SHA256 over the password, salted per account with the normalized e-mail. */
export async function deriveAuthKey(email: string, password: string, iterations = KDF_ITERATIONS): Promise<string> {
  const key = await crypto.subtle.importKey('raw', enc.encode(password), 'PBKDF2', false, ['deriveBits']);
  const salt = enc.encode(`kortex-auth-v${KDF_VERSION}:${email.trim().toLowerCase()}`);
  const bits = await crypto.subtle.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt, iterations }, key, 256);
  return b64url(new Uint8Array(bits));
}

const COMMON = new Set(['password123', '1234567890', 'azertyuiop', 'qwertyuiop', 'motdepasse', 'password12', '0123456789']);

/** The server only ever sees a derived key, so the password policy lives here. */
export function checkPasswordPolicy(password: string): 'ok' | 'too_short' | 'too_long' | 'too_common' {
  if (password.length < 10) return 'too_short';
  if (password.length > 200) return 'too_long';
  if (COMMON.has(password.toLowerCase())) return 'too_common';
  return 'ok';
}
