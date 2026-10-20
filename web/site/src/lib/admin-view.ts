// Pure helpers of the owner's dashboard: formatting and the rollback commands. No DOM here.
export interface ErrorRow { day: string; app: string; kind: string; n: number }
export interface HostRow { host: string; ok: boolean; status: number; ms: number }

export const WORKER_CONFIGS: Record<string, string> = {
  'kortex-api': 'services/api/wrangler.jsonc',
  'kortex-site': 'services/site/wrangler.jsonc',
  'kortex-run': 'services/run/wrangler.jsonc',
  'kortex-desk': 'services/desk/wrangler.jsonc',
  'kortex-hub': 'services/hub/wrangler.jsonc',
  'kortex-store': 'services/store/wrangler.jsonc',
  'kortex-seek': 'services/seek/wrangler.jsonc',
  'kortex-flow': 'services/flow/wrangler.jsonc',
  'kortex-mind': 'services/mind/wrangler.jsonc',
};

/** The command that puts a Worker back to a recorded version (to run from the repository). */
export function rollbackCommand(worker: string, versionId: string): string | null {
  const config = WORKER_CONFIGS[worker];
  if (!config || !/^[0-9a-f-]{36}$/.test(versionId)) return null;
  const env = worker === 'kortex-api' ? ' --env production' : '';
  return `wrangler rollback ${versionId} --config ${config}${env}`;
}

export const fmtDuration = (ms: number): string => (ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(1)} s`);

/** Errors of the last days, one line per app with its kinds. */
export function summarizeErrors(rows: ErrorRow[]): { app: string; total: number; kinds: Record<string, number> }[] {
  const by = new Map<string, { app: string; total: number; kinds: Record<string, number> }>();
  for (const r of rows) {
    const entry = by.get(r.app) ?? { app: r.app, total: 0, kinds: {} };
    entry.total += r.n;
    entry.kinds[r.kind] = (entry.kinds[r.kind] ?? 0) + r.n;
    by.set(r.app, entry);
  }
  return [...by.values()].sort((a, b) => b.total - a.total || a.app.localeCompare(b.app));
}

/** Width of a bar in percent, clamped so an overrun still fits. */
export const barPercent = (used: number, cap: number): number => (cap <= 0 ? 0 : Math.max(0, Math.min(100, Math.round((used / cap) * 100))));

/** `ok`, `warn` (budget above 70 %) or `bad` (above 90 %): the colour of the AI budget bar. */
export const budgetTone = (percent: number): 'ok' | 'warn' | 'bad' => (percent >= 90 ? 'bad' : percent >= 70 ? 'warn' : 'ok');
