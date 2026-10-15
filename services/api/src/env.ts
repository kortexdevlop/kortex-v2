export interface Env {
  DB: D1Database;
  ALLOWED_ORIGINS: string;
  COOKIE_NAME?: string;
  COOKIE_SECURE?: string;
  PBKDF2_ITERATIONS?: string;
  /** Workers AI binding (absent in local tests/dev unless mocked). */
  AI?: Ai;
  AI_USER_CAP?: string;
  AI_USER_REFILL_3H?: string;
  AI_GLOBAL_DAILY?: string;
}

export interface Ctx {
  env: Env;
  req: Request;
  url: URL;
  /** Set by the session guard. */
  userId?: string;
  /** Resolved client IP (Cloudflare header) used for rate limiting only. */
  ip: string;
}
