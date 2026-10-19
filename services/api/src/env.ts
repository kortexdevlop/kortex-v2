export interface Env {
  DB: D1Database;
  WEBAUTHN_RP_ID?: string;
  WEBAUTHN_ORIGIN?: string;
  TURNSTILE_SECRET_KEY?: string;
  TURNSTILE_REQUIRED?: string;
  ALLOWED_ORIGINS: string;
  COOKIE_NAME?: string;
  COOKIE_SECURE?: string;
  PBKDF2_ITERATIONS?: string;
  /** Workers AI binding (absent in local tests/dev unless mocked). */
  AI?: Ai;
  AI_USER_CAP?: string;
  AI_USER_REFILL_3H?: string;
  AI_GLOBAL_DAILY?: string;
  /** Comma-separated connector hosts the proxy may call with credentials (suffix match). */
  PROXY_CONNECTORS?: string;
  PROXY_FREE_PER_HOUR?: string;
  PROXY_CONNECTOR_PER_HOUR?: string;
  /** `OAUTH_<PROVIDER>_ID` / `OAUTH_<PROVIDER>_SECRET`: Worker secrets of the OAuth broker. */
  [secret: `OAUTH_${string}`]: string | undefined;
}

export interface Ctx {
  env: Env;
  req: Request;
  url: URL;
  /** Set by the session guard. */
  userId?: string;
  /** Set when the caller authenticated with a device token (CLI, desktop, Store). */
  deviceId?: string;
  /** Resolved client IP (Cloudflare header) used for rate limiting only. */
  ip: string;
}
