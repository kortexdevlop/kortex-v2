// Password hashing and token helpers on WebCrypto only (native, fits the 10 ms CPU budget).
const enc = new TextEncoder();

export const b64 = (bytes: Uint8Array): string => {
  let s = '';
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
};
export const unb64 = (s: string): Uint8Array => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
export const b64url = (bytes: Uint8Array): string => b64(bytes).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
export const hex = (bytes: Uint8Array): string => [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('');

export const randomBytes = (n: number): Uint8Array => crypto.getRandomValues(new Uint8Array(n));

export async function sha256Hex(text: string): Promise<string> {
  return hex(new Uint8Array(await crypto.subtle.digest('SHA-256', enc.encode(text))));
}

/** Constant-time comparison of two equal-purpose byte strings. */
export function timingSafeEqual(a: Uint8Array, b: Uint8Array): boolean {
  let diff = a.length ^ b.length;
  const n = Math.max(a.length, b.length);
  for (let i = 0; i < n; i++) diff |= (a[i] ?? 0) ^ (b[i] ?? 0);
  return diff === 0;
}

async function pbkdf2(password: string, salt: Uint8Array, iterations: number): Promise<Uint8Array> {
  const key = await crypto.subtle.importKey('raw', enc.encode(password), 'PBKDF2', false, ['deriveBits']);
  const bits = await crypto.subtle.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: salt as BufferSource, iterations }, key, 256);
  return new Uint8Array(bits);
}

/** `pbkdf2$<iterations>$<salt b64>$<hash b64>` (Workers cap PBKDF2 at 100 000 iterations). */
export async function hashPassword(password: string, iterations = 100_000): Promise<string> {
  const salt = randomBytes(16);
  const hash = await pbkdf2(password, salt, iterations);
  return `pbkdf2$${iterations}$${b64(salt)}$${b64(hash)}`;
}

export async function verifyPassword(password: string, stored: string): Promise<boolean> {
  const [scheme, iter, salt, hash] = stored.split('$');
  const iterations = Number(iter);
  if (scheme !== 'pbkdf2' || !Number.isInteger(iterations) || iterations < 1 || !salt || !hash) return false;
  const actual = await pbkdf2(password, unb64(salt), iterations);
  return timingSafeEqual(actual, unb64(hash));
}

/** Verifies against a throwaway hash so unknown accounts cost the same as wrong passwords. */
export async function burnVerification(password: string, iterations = 100_000): Promise<void> {
  await pbkdf2(password, new Uint8Array(16), iterations);
}

export const newSessionToken = (): string => b64url(randomBytes(32));
