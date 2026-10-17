import type { Ctx } from '../env.ts';
import { MODELS, estimateTokens, neurons } from '../ai-models.ts';
import { creditsView, markGloballyExhausted, reserve, settle } from '../credits.ts';
import { HttpError, errorBody, isRecord, json, readJson } from '../http.ts';
import { rateLimit } from '../ratelimit.ts';
import { requireUser } from '../session.ts';

const MAX_MESSAGES = 40;
const MAX_MESSAGE_CHARS = 8_000;
const MAX_TOTAL_CHARS = 24_000;
const ROLES = new Set(['system', 'user', 'assistant']);

interface Msg { role: string; content: string }

function parseMessages(v: unknown): Msg[] {
  if (!Array.isArray(v) || v.length === 0 || v.length > MAX_MESSAGES) throw new HttpError(400, 'invalid_messages');
  let total = 0;
  return v.map((m) => {
    if (!isRecord(m) || typeof m.role !== 'string' || !ROLES.has(m.role) || typeof m.content !== 'string' || m.content.length === 0 || m.content.length > MAX_MESSAGE_CHARS) {
      throw new HttpError(400, 'invalid_messages');
    }
    total += m.content.length;
    if (total > MAX_TOTAL_CHARS) throw new HttpError(413, 'messages_too_large');
    return { role: m.role, content: m.content };
  });
}

export async function getCredits(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  return json({ ...await creditsView(ctx.env, uid), available: Boolean(ctx.env.AI) });
}

/** POST { messages, model?: "fast" | "smart", max_tokens?, consent: true }
 *  Content is only sent to the model with explicit consent for this request (privacy rule §2.15). */
export async function chat(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  await rateLimit(ctx, `ai:${uid}`, 30, 60);
  const b = await readJson(ctx.req, 60_000);
  if (!isRecord(b)) throw new HttpError(400, 'invalid_json');
  if (b.consent !== true) throw new HttpError(400, 'consent_required');
  const messages = parseMessages(b.messages);
  const modelKey = b.model === undefined ? 'fast' : b.model;
  if (typeof modelKey !== 'string' || !(modelKey in MODELS)) throw new HttpError(400, 'invalid_model');
  const model = MODELS[modelKey]!;
  const maxOut = b.max_tokens === undefined ? 512 : b.max_tokens;
  if (!Number.isInteger(maxOut) || (maxOut as number) < 1 || (maxOut as number) > 1024) throw new HttpError(400, 'invalid_max_tokens');
  if (!ctx.env.AI) throw new HttpError(503, 'ai_unavailable');

  const inTokens = estimateTokens(messages.reduce((n, m) => n + m.content.length, 0));
  const estimate = neurons(model, inTokens, maxOut as number);
  const r = await reserve(ctx.env, uid, estimate);
  if (!r.ok) {
    return json({ ...errorBody(r.reason), credits: await creditsView(ctx.env, uid) }, { status: 429, headers: { 'Retry-After': String(r.retryAfter) } });
  }

  let out: { response?: string; usage?: { prompt_tokens?: number; completion_tokens?: number } };
  try {
    out = (await ctx.env.AI.run(model.id as never, { messages, max_tokens: maxOut } as never)) as typeof out;
  } catch (e) {
    await settle(ctx.env, uid, r.cost, 0, modelKey); // nothing was consumed: full refund
    const msg = e instanceof Error ? e.message : '';
    if (/daily|limit|quota|neuron/i.test(msg)) {
      await markGloballyExhausted(ctx.env);
      return json({ ...errorBody('ai_paused'), credits: await creditsView(ctx.env, uid) }, { status: 429 });
    }
    return json(errorBody('ai_unavailable'), { status: 502 });
  }
  const text = typeof out.response === 'string' ? out.response : '';
  const usedIn = out.usage?.prompt_tokens ?? inTokens;
  const usedOut = out.usage?.completion_tokens ?? estimateTokens(text.length);
  await settle(ctx.env, uid, r.cost, neurons(model, usedIn, usedOut), modelKey);
  return json({ content: text, credits: await creditsView(ctx.env, uid) });
}
