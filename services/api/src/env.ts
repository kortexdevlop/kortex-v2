export interface Env {
  DB: D1Database;
  ALLOWED_ORIGINS: string;
  COOKIE_NAME?: string;
  COOKIE_SECURE?: string;
  PBKDF2_ITERATIONS?: string;
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
