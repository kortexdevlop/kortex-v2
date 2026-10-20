// Checks the security headers of every public host (run against production or a preview).
//   node scripts/audit-headers.mjs [--root kortex.site]
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
const text = readFileSync(resolve(import.meta.dirname, '../brand/domains.toml'), 'utf8');
const domains = Object.fromEntries([...text.matchAll(/^([a-z]+)\s*=\s*"([^"]+)"/gm)].map(m => [m[1], m[2]]));
const hosts = Object.entries(domains).filter(([key]) => key !== 'root' || true).map(([key, host]) => ({ key, host }));
const probes = hosts.map(({ key, host }) => ({ key, url: `https://${host}/${key === 'run' ? 'calcul-tva/' : ''}`, api: key === 'api' }));
let failures = 0;
for (const probe of probes) {
  let response;
  try { response = await fetch(probe.url, { redirect: 'manual' }); } catch (e) { console.log(`✗ ${probe.url}: ${e.message}`); failures++; continue; }
  const h = name => response.headers.get(name) ?? '';
  const problems = [];
  if (!/max-age=\d{7,}/.test(h('strict-transport-security'))) problems.push('HSTS missing or short');
  if (h('x-content-type-options').toLowerCase() !== 'nosniff') problems.push('nosniff missing');
  if (h('referrer-policy') !== 'no-referrer') problems.push('referrer-policy is not no-referrer');
  if (!probe.api) {
    const csp = h('content-security-policy');
    if (!csp.includes("default-src 'self'")) problems.push("CSP without default-src 'self'");
    if (!csp.includes("object-src 'none'")) problems.push("CSP without object-src 'none'");
    if (!csp.includes('base-uri')) problems.push('CSP without base-uri');
    if (!csp.includes('frame-ancestors')) problems.push('CSP without frame-ancestors');
    if (!h('permissions-policy').includes('camera=()')) problems.push('permissions-policy missing');
  }
  if (problems.length) { failures++; console.log(`✗ ${probe.url}: ${problems.join('; ')}`); } else console.log(`✓ ${probe.url} (${response.status})`);
}
process.exit(failures ? 1 : 0);
