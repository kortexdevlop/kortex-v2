// Kortex Seek, level 1: instant, offline search over the tool index (no dependency).
// Handles accents, typos (edit distance), prefixes and a small FR/EN synonym table.

export interface SeekEntry {
  id: string;
  name: string;
  tagline: string;
  keywords: string[];
  intents: string[];
  path: string;
  glyph: string;
  variants: { name: string; path: string }[];
}
export interface SeekResult { entry: SeekEntry; score: number; variant?: { name: string; path: string } }

const SYNONYMS: Record<string, string[]> = {
  photo: ['image'], photos: ['image'], picture: ['image'], pictures: ['image'], img: ['image'], jpeg: ['jpg'],
  mdp: ['mot', 'passe', 'password'], pwd: ['password'], motdepasse: ['mot', 'passe'],
  compresser: ['compress'], reduire: ['compress'], alleger: ['compress'],
  fusionner: ['merge'], assembler: ['merge'], joindre: ['merge'], combiner: ['merge'], combine: ['merge'], join: ['merge'],
  convertir: ['convert'], transformer: ['convert'], changer: ['convert'], converter: ['convert'],
  salaire: ['brut', 'net', 'salary'], paie: ['salaire', 'brut', 'net'], payslip: ['salary'],
  empreinte: ['hash'], checksum: ['hash'], sha: ['hash'], somme: ['hash'],
  lien: ['qr'], url: ['qr'], code: ['qr'],
};

export const fold = (s: string): string =>
  s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim();

const STOP = new Set(['de', 'des', 'du', 'd', 'un', 'une', 'le', 'la', 'les', 'l', 'en', 'et', 'ou', 'a', 'au', 'aux', 'mon', 'ma', 'mes', 'ce', 'cet', 'cette', 'pour', 'sur', 'dans', 'je', 'veux', 'voudrais', 'comment', 'to', 'the', 'my', 'of', 'an', 'in', 'on', 'for', 'and', 'or', 'how', 'i', 'want', 'need', 'make', 'create']);

export const tokens = (s: string): string[] => fold(s).split(' ').filter(Boolean);

/** Query tokens without filler words (kept if the query is only filler). */
const significant = (s: string): string[] => {
  const all = tokens(s);
  const kept = all.filter((t) => !STOP.has(t));
  return kept.length ? kept : all;
};

/** Damerau-Levenshtein distance, bounded: returns max+1 as soon as it exceeds `max`. */
export function distance(a: string, b: string, max = 2): number {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  const prev2: number[] = [];
  let prev = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    const cur = [i];
    let rowMin = i;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      let v = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) v = Math.min(v, (prev2[j - 2] ?? Infinity) + 1);
      cur[j] = v;
      rowMin = Math.min(rowMin, v);
    }
    if (rowMin > max) return max + 1;
    prev2.length = 0;
    prev2.push(...prev);
    prev = cur;
  }
  return prev[b.length];
}

/** How well query token `q` matches word `w`: 0 (none) .. 1 (exact). */
function wordScore(q: string, w: string): number {
  if (q === w) return 1;
  if (w.startsWith(q) && q.length >= 2) return 0.8;
  if (q.length >= 3 && w.includes(q)) return 0.5;
  if (q.length >= 4) {
    const d = distance(q, w, q.length >= 7 ? 2 : 1);
    if (d <= 1) return 0.6;
    if (d === 2 && q.length >= 7) return 0.45;
  }
  return 0;
}

function fieldScore(qTokens: string[], text: string): number {
  const words = tokens(text);
  if (!words.length) return 0;
  let total = 0;
  for (const q of qTokens) {
    let best = 0;
    for (const w of words) best = Math.max(best, wordScore(q, w));
    if (best === 0) return 0; // every query token must match somewhere in this field
    total += best;
  }
  return total / qTokens.length;
}

function expand(q: string): string[][] {
  // each query token becomes a group of alternatives (itself + synonyms); a synonym that is
  // already a query word of its own is dropped so one word cannot count twice
  const own = significant(q);
  return own.map((t) => [t, ...(SYNONYMS[t] ?? []).filter((x) => !own.includes(x))]);
}

function scoreEntry(groups: string[][], e: SeekEntry): number {
  const fields: [string, number][] = [
    [e.name, 6],
    ...e.keywords.map((k): [string, number] => [k, 4]),
    ...e.intents.map((k): [string, number] => [k, 3]),
    [e.tagline, 1.5],
    ...e.variants.map((v): [string, number] => [v.name, 3]),
  ];
  let best = 0;
  // 1) all tokens inside a single field (strongest signal)
  for (const [text, w] of fields) {
    let sum = 0;
    let ok = true;
    for (const alts of groups) {
      const s = Math.max(...alts.map((a) => fieldScore([a], text)));
      if (s === 0) { ok = false; break; }
      sum += s;
    }
    if (ok) best = Math.max(best, (sum / groups.length) * w);
  }
  // 2) tokens spread over several fields (weaker): at least half of them must match
  if (best === 0 && groups.length > 1) {
    let sum = 0;
    let matched = 0;
    for (const alts of groups) {
      let g = 0;
      for (const [text, w] of fields) {
        // synonyms count less than the word typed; weak (substring) matches do not count at all
        const fs = Math.max(...alts.map((a, i) => fieldScore([a], text) * (i === 0 ? 1 : 0.8)));
        if (fs >= 0.6) g = Math.max(g, fs * w);
      }
      if (g > 0) { matched++; sum += g; }
    }
    if (matched / groups.length >= 0.6) best = (sum / groups.length) * 0.6;
  }
  return best;
}

export function search(index: SeekEntry[], query: string, limit = 8): SeekResult[] {
  const groups = expand(query);
  if (!groups.length) return [];
  const out: SeekResult[] = [];
  for (const entry of index) {
    const score = scoreEntry(groups, entry);
    if (score <= 0) continue;
    // A variant is only preferred when the query is specific enough (2+ words).
    const variant = groups.length < 2 ? undefined : entry.variants.find((v) => groups.every((alts) => alts.some((a) => fieldScore([a], v.name) >= 0.8)));
    out.push({ entry, score: variant ? score + 0.5 : score, variant });
  }
  return out.sort((a, b) => b.score - a.score || a.entry.name.localeCompare(b.entry.name)).slice(0, limit);
}
