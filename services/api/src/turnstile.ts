import type { Ctx } from './env.ts';
import { HttpError } from './http.ts';

export async function verifyRegistrationGuard(ctx: Ctx, token: unknown): Promise<void> {
  if (ctx.env.TURNSTILE_REQUIRED !== 'true') return;
  if (!ctx.env.TURNSTILE_SECRET_KEY) throw new HttpError(503,'registration_unavailable');
  if (typeof token !== 'string' || !token || token.length > 2048) throw new HttpError(400,'challenge_required');
  let data: {success?: boolean; hostname?: string; action?: string};
  try {
    const response = await fetch('https://challenges.cloudflare.com/turnstile/v0/siteverify', {
      method:'POST', headers:{'Content-Type':'application/json'},
      body:JSON.stringify({secret:ctx.env.TURNSTILE_SECRET_KEY,response:token,remoteip:ctx.ip}),
      signal:AbortSignal.timeout(8000),
    });
    if (!response.ok) throw new Error('siteverify unavailable');
    data = await response.json();
  } catch { throw new HttpError(503,'registration_unavailable'); }
  const expected = new URL(ctx.env.WEBAUTHN_ORIGIN ?? 'http://localhost:4321').hostname;
  if (data.success !== true || data.hostname !== expected || data.action !== 'register') throw new HttpError(400,'challenge_required');
}
