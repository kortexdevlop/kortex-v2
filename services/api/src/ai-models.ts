// Neuron prices per million tokens, from the Workers AI pricing page (checked 2026-10-15).
// Re-check when changing models: prices differ a lot between model sizes.
export interface ModelInfo { id: string; inPerM: number; outPerM: number }

export const MODELS: Record<string, ModelInfo> = {
  fast: { id: '@cf/meta/llama-3.2-3b-instruct', inPerM: 4_625, outPerM: 30_475 },
  smart: { id: '@cf/meta/llama-3.1-8b-instruct-fp8-fast', inPerM: 4_119, outPerM: 34_868 },
};

/** Cost in neurons of `inTokens` prompt tokens and `outTokens` generated tokens. */
export function neurons(m: ModelInfo, inTokens: number, outTokens: number): number {
  return (inTokens * m.inPerM + outTokens * m.outPerM) / 1_000_000;
}

/** Cheap token estimate (conservative: ~3 characters per token). */
export const estimateTokens = (chars: number): number => Math.ceil(chars / 3);
