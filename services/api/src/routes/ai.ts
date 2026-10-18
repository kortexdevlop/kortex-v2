import type { Ctx } from '../env.ts';
import { DEFAULT_MODEL, MODELS, estimateTokens, neurons, type ModelInfo } from '../ai-models.ts';
import { creditsView, markGloballyExhausted, reserve, settle } from '../credits.ts';
import { HttpError, errorBody, isRecord, json, readJson } from '../http.ts';
import { rateLimit } from '../ratelimit.ts';
import { requireUser } from '../session.ts';

const MAX_MESSAGES = 60;
const MAX_MESSAGE_CHARS = 8_000;
const MAX_TOTAL_CHARS = 48_000;
const MAX_TOOLS = 24;
const MAX_TOOLS_CHARS = 40_000;
const MAX_OUT_TOKENS = 2_048;
const DEFAULT_OUT_TOKENS = 600;
const NAME = /^[A-Za-z0-9_-]{1,64}$/;
const ROLES = new Set(['system', 'user', 'assistant', 'tool']);

interface ToolCall { id: string; type: 'function'; function: { name: string; arguments: string } }
interface Msg { role: string; content: string | null; tool_calls?: ToolCall[]; tool_call_id?: string }
interface Tool { type: 'function'; function: { name: string; description: string; parameters: Record<string, unknown> } }

function parseToolCalls(v: unknown): ToolCall[] {
  if (!Array.isArray(v) || v.length === 0 || v.length > 8) throw new HttpError(400, 'invalid_messages');
  return v.map((c) => {
    if (!isRecord(c) || typeof c.id !== 'string' || !NAME.test(c.id) || !isRecord(c.function)
      || typeof c.function.name !== 'string' || !NAME.test(c.function.name)
      || typeof c.function.arguments !== 'string' || c.function.arguments.length > MAX_MESSAGE_CHARS) {
      throw new HttpError(400, 'invalid_messages');
    }
    return { id: c.id, type: 'function', function: { name: c.function.name, arguments: c.function.arguments } };
  });
}

/** Chars counted against the prompt budget, and the validated conversation. */
function parseMessages(v: unknown): { messages: Msg[]; chars: number } {
  if (!Array.isArray(v) || v.length === 0 || v.length > MAX_MESSAGES) throw new HttpError(400, 'invalid_messages');
  let total = 0;
  const messages = v.map((m): Msg => {
    if (!isRecord(m) || typeof m.role !== 'string' || !ROLES.has(m.role)) throw new HttpError(400, 'invalid_messages');
    const text = m.content;
    if (text !== null && text !== undefined && (typeof text !== 'string' || text.length > MAX_MESSAGE_CHARS)) throw new HttpError(400, 'invalid_messages');
    if (m.role === 'assistant') {
      // An assistant turn is either text or tool calls (or both); never empty.
      const calls = m.tool_calls === undefined ? undefined : parseToolCalls(m.tool_calls);
      if (!calls && !text) throw new HttpError(400, 'invalid_messages');
      total += (text ?? '').length + (calls ? JSON.stringify(calls).length : 0);
      return { role: 'assistant', content: text ?? null, ...(calls ? { tool_calls: calls } : {}) };
    }
    if (!text) throw new HttpError(400, 'invalid_messages');
    if (m.role === 'tool') {
      if (typeof m.tool_call_id !== 'string' || !NAME.test(m.tool_call_id)) throw new HttpError(400, 'invalid_messages');
      total += text.length;
      return { role: 'tool', content: text, tool_call_id: m.tool_call_id };
    }
    total += text.length;
    return { role: m.role, content: text };
  });
  if (total > MAX_TOTAL_CHARS) throw new HttpError(413, 'messages_too_large');
  return { messages, chars: total };
}

function parseTools(v: unknown): { tools: Tool[]; chars: number } | undefined {
  if (v === undefined) return undefined;
  if (!Array.isArray(v) || v.length === 0 || v.length > MAX_TOOLS) throw new HttpError(400, 'invalid_tools');
  const tools = v.map((t): Tool => {
    if (!isRecord(t) || t.type !== 'function' || !isRecord(t.function)) throw new HttpError(400, 'invalid_tools');
    const f = t.function;
    if (typeof f.name !== 'string' || !NAME.test(f.name) || typeof f.description !== 'string' || f.description.length > 600 || !isRecord(f.parameters)) {
      throw new HttpError(400, 'invalid_tools');
    }
    return { type: 'function', function: { name: f.name, description: f.description, parameters: f.parameters } };
  });
  const chars = JSON.stringify(tools).length;
  if (chars > MAX_TOOLS_CHARS) throw new HttpError(413, 'tools_too_large');
  return { tools, chars };
}

export async function getCredits(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  return json({ ...await creditsView(ctx.env, uid), available: Boolean(ctx.env.AI) });
}

interface Usage { prompt_tokens?: number; completion_tokens?: number; neurons?: number }
const used = (u: Usage | undefined, model: ModelInfo, fallbackIn: number, fallbackOut: number): number =>
  typeof u?.neurons === 'number' && u.neurons >= 0
    ? u.neurons
    : neurons(model, u?.prompt_tokens ?? fallbackIn, u?.completion_tokens ?? fallbackOut);

/** The slice of a provider chunk the client needs: nothing else leaves the gateway. */
function clientDelta(chunk: unknown): { delta: Record<string, unknown>; finish_reason: string | null } | null {
  if (!isRecord(chunk) || !Array.isArray(chunk.choices) || !isRecord(chunk.choices[0])) return null;
  const choice = chunk.choices[0];
  const delta = isRecord(choice.delta) ? choice.delta : {};
  const out: Record<string, unknown> = {};
  if (typeof delta.content === 'string' && delta.content) out.content = delta.content;
  if (Array.isArray(delta.tool_calls)) out.tool_calls = delta.tool_calls;
  const finish = typeof choice.finish_reason === 'string' ? choice.finish_reason : null;
  return Object.keys(out).length === 0 && finish === null ? null : { delta: out, finish_reason: finish };
}

const sse = (event: string | null, data: unknown): Uint8Array =>
  new TextEncoder().encode(`${event ? `event: ${event}\n` : ''}data: ${JSON.stringify(data)}\n\n`);

/** POST { messages, tools?, tool_choice?, stream?, max_tokens?, consent: true }
 *  Content is only sent to the model with explicit consent for this request (privacy rule §2.15).
 *  Thinking is always off: it multiplied the cost by ~10 on real calls for no gain in tool use. */
export async function chat(ctx: Ctx): Promise<Response> {
  const uid = await requireUser(ctx);
  await rateLimit(ctx, `ai:${uid}`, 30, 60);
  const b = await readJson(ctx.req, 160_000);
  if (!isRecord(b)) throw new HttpError(400, 'invalid_json');
  if (b.consent !== true) throw new HttpError(400, 'consent_required');
  const { messages, chars } = parseMessages(b.messages);
  const parsedTools = parseTools(b.tools);
  const modelKey = b.model === undefined ? DEFAULT_MODEL : b.model;
  if (typeof modelKey !== 'string' || !(modelKey in MODELS)) throw new HttpError(400, 'invalid_model');
  const model = MODELS[modelKey]!;
  const maxOut = b.max_tokens === undefined ? DEFAULT_OUT_TOKENS : b.max_tokens;
  if (!Number.isInteger(maxOut) || (maxOut as number) < 1 || (maxOut as number) > MAX_OUT_TOKENS) throw new HttpError(400, 'invalid_max_tokens');
  const toolChoice = b.tool_choice === undefined ? 'auto' : b.tool_choice;
  if (!['auto', 'none', 'required'].includes(String(toolChoice)) || (b.tool_choice !== undefined && !parsedTools)) throw new HttpError(400, 'invalid_tool_choice');
  if (b.stream !== undefined && typeof b.stream !== 'boolean') throw new HttpError(400, 'invalid_stream');
  if (!ctx.env.AI) throw new HttpError(503, 'ai_unavailable');

  const inTokens = estimateTokens(chars + (parsedTools?.chars ?? 0));
  const estimate = neurons(model, inTokens, maxOut as number);
  const r = await reserve(ctx.env, uid, estimate);
  if (!r.ok) {
    return json({ ...errorBody(r.reason), credits: await creditsView(ctx.env, uid) }, { status: 429, headers: { 'Retry-After': String(r.retryAfter) } });
  }
  const input = {
    messages,
    max_completion_tokens: maxOut,
    chat_template_kwargs: { enable_thinking: false },
    ...(parsedTools ? { tools: parsedTools.tools, tool_choice: toolChoice } : {}),
    ...(b.stream === true ? { stream: true, stream_options: { include_usage: true } } : {}),
  };

  let out: unknown;
  try {
    out = await ctx.env.AI.run(model.id as never, input as never);
  } catch (e) {
    await settle(ctx.env, uid, r.cost, 0, modelKey); // nothing was consumed: full refund
    return providerFailure(ctx, uid, e);
  }

  if (b.stream === true && out instanceof ReadableStream) return streamResponse(ctx, uid, out as ReadableStream<Uint8Array>, r.cost, modelKey, model, inTokens);

  const result = isRecord(out) ? out : {};
  const choice = Array.isArray(result.choices) && isRecord(result.choices[0]) ? result.choices[0] : {};
  const message = isRecord(choice.message) ? choice.message : {};
  const content = typeof message.content === 'string' ? message.content : null;
  const toolCalls = Array.isArray(message.tool_calls) && message.tool_calls.length > 0 ? message.tool_calls : undefined;
  const usage = isRecord(result.usage) ? (result.usage as Usage) : undefined;
  await settle(ctx.env, uid, r.cost, used(usage, model, inTokens, estimateTokens((content ?? '').length)), modelKey);
  return json({
    message: { role: 'assistant', content, ...(toolCalls ? { tool_calls: toolCalls } : {}) },
    finish_reason: typeof choice.finish_reason === 'string' ? choice.finish_reason : 'stop',
    credits: await creditsView(ctx.env, uid),
  });
}

async function providerFailure(ctx: Ctx, uid: string, e: unknown): Promise<Response> {
  const msg = e instanceof Error ? e.message : '';
  if (/daily|limit|quota|neuron/i.test(msg)) {
    await markGloballyExhausted(ctx.env);
    return json({ ...errorBody('ai_paused'), credits: await creditsView(ctx.env, uid) }, { status: 429 });
  }
  return json(errorBody('ai_unavailable'), { status: 502 });
}

/** Re-emits the provider's SSE as `data: {delta, finish_reason}` events, then `event: done` with the
 *  credits. The reservation is settled exactly once: on the provider's reported neurons at the
 *  end, or on a character-based estimate if the client leaves early or the provider fails. */
function streamResponse(ctx: Ctx, uid: string, source: ReadableStream<Uint8Array>, reserved: number, modelKey: string, model: ModelInfo, inTokens: number): Response {
  const reader = source.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let outChars = 0;
  let usage: Usage | undefined;
  let settled = false;
  const finish = async (): Promise<void> => {
    if (settled) return;
    settled = true;
    await settle(ctx.env, uid, reserved, used(usage, model, inTokens, estimateTokens(outChars)), modelKey);
  };
  const handle = (line: string, controller: ReadableStreamDefaultController<Uint8Array>): void => {
    if (!line.startsWith('data:')) return;
    const data = line.slice(5).trim();
    if (!data || data === '[DONE]') return;
    let chunk: unknown;
    try { chunk = JSON.parse(data); } catch { return; }
    if (isRecord(chunk) && isRecord(chunk.usage) && typeof (chunk.usage as Usage).neurons === 'number') usage = chunk.usage as Usage;
    const piece = clientDelta(chunk);
    if (!piece) return;
    const text = typeof piece.delta.content === 'string' ? piece.delta.content : '';
    outChars += text.length + (piece.delta.tool_calls ? JSON.stringify(piece.delta.tool_calls).length : 0);
    controller.enqueue(sse(null, piece));
  };
  const body = new ReadableStream<Uint8Array>({
    // `pull` must emit or finish: a provider chunk that yields no client event (role, usage) would
    // otherwise leave the consumer waiting forever, so keep reading until something is queued.
    async pull(controller) {
      try {
        for (;;) {
          const before = controller.desiredSize;
          const { done, value } = await reader.read();
          if (done) {
            if (buffer) handle(buffer, controller);
            await finish();
            controller.enqueue(sse('done', { credits: await creditsView(ctx.env, uid) }));
            controller.close();
            return;
          }
          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split('\n');
          buffer = lines.pop() ?? '';
          for (const line of lines) handle(line, controller);
          if (controller.desiredSize !== before) return; // something was enqueued
        }
      } catch {
        await finish();
        controller.enqueue(sse('error', { error: 'ai_unavailable' }));
        controller.close();
      }
    },
    async cancel() {
      await reader.cancel().catch(() => undefined);
      await finish();
    },
  });
  return new Response(body, { headers: { 'Content-Type': 'text/event-stream; charset=utf-8', 'Cache-Control': 'no-store' } });
}
