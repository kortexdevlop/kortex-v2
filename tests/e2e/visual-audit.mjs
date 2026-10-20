// Visual audit of the built site: every page at 375/768/1280/1920 px, light and dark.
// Checks horizontal overflow, clipped text, touch targets under 44 px, console errors and,
// when AXE_PATH points to axe-core's axe.min.js, WCAG 2.1 AA violations (contrast included).
// Usage: node tests/e2e/visual-audit.mjs [base-url] (default http://localhost:4321); writes
// screenshots to AUDIT_OUT when set. Exit code 1 when a blocking defect is found.
import { chromium } from 'playwright';
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';

const base = process.argv[2] ?? 'http://localhost:4321';
const out = process.env.AUDIT_OUT;
const axePath = process.env.AXE_PATH;
const pages = (process.env.AUDIT_PAGES ?? [
  '/', '/en', '/outils', '/en/tools', '/calc', '/calcul-pourcentage', '/en/percentage-calculator', '/calculateur-hash',
  '/applications', '/en/apps', '/flow', '/en/desk', '/telecharger', '/en/download', '/compte', '/en/account',
  '/admin', '/design', '/confidentialite', '/en/privacy', '/nope-404',
].join(',')).split(',');
const widths = [375, 768, 1280, 1920];
const themes = ['light', 'dark'];

if (out) mkdirSync(out, { recursive: true });
const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || undefined });
const rows = [];
let blocking = 0;
for (const theme of themes) {
  for (const width of widths) {
    const context = await browser.newContext({ viewport: { width, height: 900 }, colorScheme: theme, reducedMotion: 'reduce' });
    // Third-party hosts (account API, Turnstile) are not part of the visual audit.
    await context.route(/^https?:\/\/(?!localhost)/, (route) => route.abort());
    for (const path of pages) {
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', (e) => errors.push(e.message));
      page.on('console', (m) => m.type() === 'error' && !/ERR_FAILED|blocked by CORS|net::/.test(m.text()) && !(path.startsWith('/nope') && /status of 404/.test(m.text())) && errors.push(m.text()));
      await page.goto(base + path, { waitUntil: 'networkidle' });
      const metrics = await page.evaluate(() => {
        const vw = document.documentElement.clientWidth;
        const overflow = document.documentElement.scrollWidth - vw;
        const visible = (e) => { const s = getComputedStyle(e); const r = e.getBoundingClientRect(); return s.visibility !== 'hidden' && s.display !== 'none' && r.width > 0 && r.height > 0; };
        const offenders = [...document.querySelectorAll('body *')].filter((e) => {
          if (!visible(e)) return false;
          const r = e.getBoundingClientRect();
          if (r.right <= vw + 1) return false;
          for (let p = e.parentElement; p; p = p.parentElement) { const o = getComputedStyle(p).overflowX; if (o === 'auto' || o === 'scroll' || o === 'hidden' || o === 'clip') return false; }
          return getComputedStyle(e).position !== 'fixed';
        }).slice(0, 5).map((e) => `${e.tagName.toLowerCase()}.${[...e.classList].join('.')}`);
        const small = [...document.querySelectorAll('a[href], button, select, input:not([type=hidden]), summary, [role=switch]')].filter((e) => {
          if (!visible(e) || e.closest('dialog:not([open]), [hidden], .prose p, .prose li, p, li > a:only-child')) return false;
          const r = e.getBoundingClientRect();
          const label = e.closest('label');
          if (label && label.getBoundingClientRect().height >= 43.5) return false; // the label is the target
          return r.height < 43.5 && !(e.tagName === 'A' && getComputedStyle(e).display === 'inline');
        }).slice(0, 6).map((e) => `${e.tagName.toLowerCase()}.${[...e.classList].join('.')}:${Math.round(e.getBoundingClientRect().height)}`);
        const tiny = [...document.querySelectorAll('main *, header *, footer *')].filter((e) => visible(e) && e.childNodes.length && [...e.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim()) && parseFloat(getComputedStyle(e).fontSize) < 13.5).slice(0, 5).map((e) => `${e.tagName.toLowerCase()}.${[...e.classList].join('.')}:${getComputedStyle(e).fontSize}`);
        return { overflow, offenders, small, tiny };
      });
      let axe = [];
      if (axePath) {
        await page.addScriptTag({ path: axePath });
        axe = await page.evaluate(async () => {
          const r = await window.axe.run(document, { runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21aa'] } });
          return r.violations.map((v) => `${v.id}(${v.nodes.length}): ${v.nodes.slice(0, 2).map((n) => n.target.join(' ')).join(' | ')}`);
        });
      }
      const bad = metrics.overflow > 0 || metrics.offenders.length > 0 || errors.length > 0 || axe.length > 0;
      if (bad) blocking++;
      rows.push({ theme, width, path, ...metrics, errors, axe });
      if (out && (width === 375 || width === 1280)) await page.screenshot({ path: join(out, `${theme}-${width}${path.replaceAll('/', '_') || '_home'}.png`), fullPage: false });
      await page.close();
    }
    await context.close();
  }
}
await browser.close();
for (const r of rows) {
  const notes = [
    r.overflow > 0 && `overflow ${r.overflow}px`, r.offenders.length && `beyond: ${r.offenders.join(' ')}`,
    r.small.length && `targets<44: ${r.small.join(' ')}`, r.tiny.length && `text<14: ${r.tiny.join(' ')}`,
    r.errors.length && `console: ${r.errors.join(' | ')}`, r.axe.length && `axe: ${r.axe.join(' ; ')}`,
  ].filter(Boolean);
  console.log(`${r.theme.padEnd(5)} ${String(r.width).padStart(4)} ${r.path.padEnd(28)} ${notes.length ? notes.join(' · ') : 'OK'}`);
}
console.log(`${rows.length} views, ${blocking} with blocking defects`);
process.exit(blocking ? 1 : 0);
