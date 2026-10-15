import type { Env } from './env.ts';

export class HttpError extends Error {
  readonly status: number;
  readonly code: string;
  constructor(status: number, code: string, message?: string) {
    super(message ?? code);
    this.status = status;
    this.code = code;
  }
}

export const json = (data: unknown, init: ResponseInit = {}): Response => {
  const headers = new Headers(init.headers);
  headers.set('Content-Type', 'application/json; charset=utf-8');
  headers.set('Cache-Control', 'no-store');
  headers.set('X-Content-Type-Options', 'nosniff');
  return new Response(JSON.stringify(data), { ...init, headers });
};

export const errorBody = (code: string): { error: string } => ({ error: code });

export const allowedOrigins = (env: Env): string[] =>
  env.ALLOWED_ORIGINS.split(',').map((s) => s.trim()).filter(Boolean);

/** Reads a JSON body with a hard size cap; any malformed input is a 400, never a 500. */
export async function readJson(req: Request, maxBytes = 70_000): Promise<unknown> {
  const len = Number(req.headers.get('content-length') ?? '0');
  if (len > maxBytes) throw new HttpError(413, 'payload_too_large');
  const text = await req.text();
  if (text.length > maxBytes) throw new HttpError(413, 'payload_too_large');
  try {
    return JSON.parse(text);
  } catch {
    throw new HttpError(400, 'invalid_json');
  }
}

export const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

export function cookie(env: Env, value: string, maxAgeSeconds: number): string {
  const name = env.COOKIE_NAME ?? 'kortex_session';
  const secure = env.COOKIE_SECURE === 'false' ? '' : '; Secure';
  // Host-only (no Domain attribute): never sent to run.kortex.site or any other sub-domain.
  return `${name}=${value}; Path=/; HttpOnly${secure}; SameSite=Lax; Max-Age=${maxAgeSeconds}`;
}

export function readCookie(env: Env, req: Request): string | null {
  const name = env.COOKIE_NAME ?? 'kortex_session';
  for (const part of (req.headers.get('cookie') ?? '').split(';')) {
    const [k, ...v] = part.trim().split('=');
    if (k === name) return v.join('=') || null;
  }
  return null;
}
