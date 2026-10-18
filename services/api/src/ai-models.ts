// Workers AI model used by the gateway. Gemma 4 26B A4B: 256k context, native function calling.
// Price: $0.10 / M input tokens and $0.30 / M output tokens (model page, checked 2026-10-18);
// Workers AI bills 1 000 neurons = $0.011, hence 9 091 and 27 273 neurons per million tokens.
// The model's own responses report `usage.neurons` (verified on real calls: it matches these
// prices), which is what we settle on; the prices only drive the reservation and the fallback.
export interface ModelInfo { id: string; inPerM: number; outPerM: number }

export const MODELS: Record<string, ModelInfo> = {
  gemma: { id: '@cf/google/gemma-4-26b-a4b-it', inPerM: 9_091, outPerM: 27_273 },
};

export const DEFAULT_MODEL = 'gemma';

/** Cost in neurons of `inTokens` prompt tokens and `outTokens` generated tokens. */
export function neurons(m: ModelInfo, inTokens: number, outTokens: number): number {
  return (inTokens * m.inPerM + outTokens * m.outPerM) / 1_000_000;
}

/** Cheap token estimate (conservative: ~3 characters per token). */
export const estimateTokens = (chars: number): number => Math.ceil(chars / 3);
