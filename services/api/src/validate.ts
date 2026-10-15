import { HttpError } from './http.ts';

const EMAIL = /^[^\s@]{1,64}@[^\s@]{1,190}\.[^\s@]{2,}$/;
export function normalizeEmail(v: unknown): string {
  if (typeof v !== 'string') throw new HttpError(400, 'invalid_email');
  const email = v.trim().toLowerCase();
  if (email.length > 254 || !EMAIL.test(email)) throw new HttpError(400, 'invalid_email');
  return email;
}

/** The client sends a derived key (ADR 0005): 32 bytes, base64url without padding = 43 characters. */
export function checkAuthKey(v: unknown, kdf: unknown): string {
  if (kdf !== 1) throw new HttpError(400, 'unsupported_kdf');
  if (typeof v !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(v)) throw new HttpError(400, 'invalid_auth_key');
  return v;
}

/** Keys are short, URL-safe identifiers: `desk.layouts`, `flow.workflow-42`, … */
export function checkKey(v: string): string {
  if (!/^[a-z0-9][a-z0-9._-]{0,63}$/.test(v)) throw new HttpError(400, 'invalid_key');
  return v;
}

export const MAX_BLOB_BYTES = 64 * 1024;

export function checkBlob(data: unknown): string {
  const text = JSON.stringify(data);
  if (text === undefined) throw new HttpError(400, 'invalid_data');
  if (new TextEncoder().encode(text).length > MAX_BLOB_BYTES) throw new HttpError(413, 'blob_too_large');
  return text;
}

export function checkTool(v: unknown): string {
  if (typeof v !== 'string' || !/^[a-z0-9]+\.[a-z0-9-]+$/.test(v) || v.length > 80) throw new HttpError(400, 'invalid_tool');
  return v;
}
