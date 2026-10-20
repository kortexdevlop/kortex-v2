import type { APIRoute } from 'astro';
import { tools, universes, universePath, toolPath, variantPath } from '../lib/data';
import { apps } from '../lib/apps';
import { legalPages } from '../lib/legal';

// Draft pages (noindex) are not listed.
const legal = legalPages.filter((p) => !p.draft);

export const GET: APIRoute = ({ site }) => {
  const base = site?.origin ?? 'https://kortex.site';
  const pairs: [string, string][] = [
    ['/', '/en'],
    ['/outils', '/en/tools'],
    ['/applications', '/en/apps'],
    ...apps.filter(app => app.available).map((app): [string,string] => [`/${app.id}`, `/en/${app.id}`]),
    ['/telecharger', '/en/download'],
    ...universes.filter(u => tools.some(t => t.tool.universe === u.id && t.tool.status !== 'draft')).map((u): [string,string] => [universePath(u,'fr'),universePath(u,'en')]),
    ...legal.map((p): [string, string] => [`/${p.slug.fr}`, `/en/${p.slug.en}`]),
    ...tools.flatMap((t): [string, string][] => [
      [toolPath(t, 'fr'), toolPath(t, 'en')],
      ...t.variants.map((v): [string, string] => [variantPath(t, v, 'fr'), variantPath(t, v, 'en')]),
    ]),
  ];
  const entry = ([fr, en]: [string, string]) =>
    [fr, en]
      .map(
        (loc) => `<url><loc>${base}${loc}</loc><xhtml:link rel="alternate" hreflang="fr" href="${base}${fr}"/><xhtml:link rel="alternate" hreflang="en" href="${base}${en}"/></url>`,
      )
      .join('');
  const body = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">${pairs.map(entry).join('')}</urlset>\n`;
  return new Response(body, { headers: { 'Content-Type': 'application/xml' } });
};
