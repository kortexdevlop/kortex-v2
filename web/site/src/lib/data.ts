import data from '../generated/site-data.json';

export type Lang = 'fr' | 'en';
type Localized = Record<string, string>;
export interface Section { title: string; text: string; faq: { q: string; a: string }[] }
export interface ToolEntry {
  tool: { id: string; version: string; universe: string; status: string; name: Localized; tagline: Localized; slug: Localized; platforms: string[]; offline: boolean; ai: boolean; viral_priority: number };
  variants: { id: string; slug: Localized; name: Localized }[];
  glyph: string;
  content: Record<Lang, Section[]>;
}
export interface Universe { id: string; name: string; hue: number }

export const tools = data.tools as unknown as ToolEntry[];
export const universes = data.universes as Universe[];
export const langs: Lang[] = ['fr', 'en'];

export const prefix = (lang: Lang) => (lang === 'fr' ? '' : '/en');
export const toolPath = (t: ToolEntry, lang: Lang) => `${prefix(lang)}/${t.tool.slug[lang]}`;
export const variantPath = (t: ToolEntry, v: ToolEntry['variants'][number], lang: Lang) => `${toolPath(t, lang)}/${v.slug[lang]}`;
export const byPriority = (a: ToolEntry, b: ToolEntry) => a.tool.viral_priority - b.tool.viral_priority;

export const ui = {
  fr: { open: 'Ouvrir', tools: 'Outils', all: 'Tous les outils', local: '100 % local', offline: 'Hors ligne', free: 'Gratuit', how: 'Comment ça marche', related: 'Variantes', empty: 'Les premiers outils arrivent bientôt.', tagline: 'Des outils gratuits, rapides, privés.', sub: 'Vos fichiers ne quittent jamais votre appareil.' },
  en: { open: 'Open', tools: 'Tools', all: 'All tools', local: '100% local', offline: 'Works offline', free: 'Free', how: 'How it works', related: 'Variants', empty: 'The first tools are coming soon.', tagline: 'Free, fast, private tools.', sub: 'Your files never leave your device.' },
} as const;
