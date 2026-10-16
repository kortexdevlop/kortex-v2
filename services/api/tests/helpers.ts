import { readFileSync } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';
import { join } from 'node:path';
import { handle } from '../src/index.ts';
import { deriveAuthKey } from '../../shared/src/auth.ts';
import type { Env } from '../src/env.ts';

/** Minimal D1 look-alike over node:sqlite (same prepare/bind/first/all/run surface). */
class Stmt {
  private args: unknown[] = [];
  private db: DatabaseSync;
  private sql: string;
  constructor(db: DatabaseSync, sql: string) {
    this.db = db;
    this.sql = sql;
  }
  bind(...a: unknown[]): this { this.args = a; return this; }
  async first<T>(): Promise<T | null> { return (this.db.prepare(this.sql).get(...(this.args as never[])) as T | undefined) ?? null; }
  async all<T>(): Promise<{ results: T[] }> { return { results: this.db.prepare(this.sql).all(...(this.args as never[])) as T[] }; }
  async run(): Promise<{ meta: { changes: number } }> {
    const r = this.db.prepare(this.sql).run(...(this.args as never[]));
    return { meta: { changes: Number(r.changes) } };
  }
}

export function makeEnv(extra: Partial<Env> = {}): { env: Env; raw: DatabaseSync } {
  const raw = new DatabaseSync(':memory:');
  raw.exec('PRAGMA foreign_keys = ON');
  for (const f of ['0001_init.sql', '0002_ai.sql', '0003_devices.sql']) raw.exec(readFileSync(join(import.meta.dirname, '../migrations', f), 'utf8'));
  const DB = { prepare: (sql: string) => new Stmt(raw, sql) } as unknown as D1Database;
  return {
    raw,
    env: {
      DB,
      ALLOWED_ORIGINS: 'https://kortex.site,https://workspace.kortex.site',
      COOKIE_NAME: 'kortex_session',
      COOKIE_SECURE: 'true',
      PBKDF2_ITERATIONS: '1000', // fast in tests; production uses 100 000
      ...extra,
    },
  };
}

export interface CallOpts { cookie?: string; origin?: string | null; raw?: string; headers?: Record<string, string> }

export async function call(env: Env, method: string, path: string, body?: unknown, opts: CallOpts = {}) {
  const headers: Record<string, string> = { ...(opts.headers ?? {}) };
  const origin = opts.origin === undefined ? 'https://kortex.site' : opts.origin;
  if (origin) headers.Origin = origin;
  if (opts.cookie) headers.Cookie = opts.cookie;
  const payload = opts.raw ?? (body === undefined ? undefined : JSON.stringify(body));
  if (payload !== undefined) headers['Content-Type'] = 'application/json';
  const res = await handle(new Request(`https://api.kortex.site${path}`, { method, headers, body: payload }), env);
  const text = await res.text();
  let data: any = null;
  try { data = text ? JSON.parse(text) : null; } catch { data = text; }
  return { status: res.status, data, headers: res.headers };
}

/** Client-side derivation with few iterations (tests only; real clients use 600 000). */
export async function creds(email: string, password: string) {
  return { email, auth_key: await deriveAuthKey(email, password, 1000), kdf: 1 };
}

/** Registers a user and returns the session cookie (name=value) to send back. */
export async function signup(env: Env, email = 'a@example.com', password = 'correct horse battery') {
  const r = await call(env, 'POST', '/v1/auth/register', await creds(email, password));
  const setCookie = r.headers.get('set-cookie') ?? '';
  return { ...r, cookie: setCookie.split(';')[0] ?? '' };
}
