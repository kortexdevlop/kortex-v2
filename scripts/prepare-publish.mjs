// Prepare only public static artifacts. Credentials and application sources are never copied.
import { readFileSync, writeFileSync, mkdirSync, rmSync, cpSync, readdirSync, statSync } from 'node:fs';
import { publicFeatures } from './public-features.mjs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { resolve, join, basename, relative, sep } from 'node:path';
const root = resolve(import.meta.dirname, '..');
// The owner's dashboard lists the recent publications: refresh that data with every deploy.
spawnSync('node', [join(root, 'scripts/build-admin-data.mjs')], { stdio: 'inherit' });
const domains = Object.fromEntries([...readFileSync(join(root, 'brand/domains.toml'), 'utf8').matchAll(/^\s*(\w+)\s*=\s*"([^"]+)"/gm)].map(m => [m[1], m[2]]));
const features = publicFeatures(root);
const security = JSON.parse(readFileSync(join(root,'brand/security.json'),'utf8'));
const turnstile = security.turnstile_site_key ? ' https://challenges.cloudflare.com' : '';
const registry = JSON.parse(readFileSync(join(root, 'registry/contracts.json'), 'utf8'));
for (const {tool} of registry.tools) {
  for (const file of ['index.html', 'sw.js']) statSync(join(root, 'dist/run', tool.id, file));
}
if (features.account) {
  const path = join(root, 'services/api/wrangler.jsonc');
  const config = JSON.parse(readFileSync(path, 'utf8'));
  config.env.production.routes = [{pattern: domains.api, custom_domain: true}];
  config.env.production.vars.WEBAUTHN_RP_ID = domains.root;
  config.env.production.vars.WEBAUTHN_ORIGIN = `https://${domains.root}`;
  config.env.production.vars.TURNSTILE_REQUIRED = security.turnstile_site_key ? 'true' : 'false';
  config.env.production.vars.ALLOWED_ORIGINS = Object.entries(domains).filter(([key]) => !['api','run'].includes(key)).map(([,host]) => `https://${host}`).join(',');
  writeFileSync(path, JSON.stringify(config, null, 2) + '\n');
}
const summary = {};
const targets = [['site', 'web/site/dist', domains.root], ['run', 'dist/run', domains.run]];
const appRegistry = JSON.parse(readFileSync(join(root, 'registry/apps.json'), 'utf8'));
for (const {app} of appRegistry.apps.filter(({app}) => ['beta', 'stable'].includes(app.status) && app.platforms.includes('web'))) {
  const name = app.id.replace(/^apps\./, '');
  if (!domains[name]) throw new Error(`Missing application domain: ${app.id}`);
  const receipt = JSON.parse(readFileSync(join(root, `.kortex/app-receipts/apps.${name}-web.json`), 'utf8'));
  for (const [path, hash] of Object.entries(receipt.files ?? {})) {
    if (createHash('sha256').update(readFileSync(join(root, 'dist/apps', `apps.${name}`, path))).digest('hex') !== hash) throw new Error(`Stale app artifact: ${name}/${path}`);
  }
  if (!Object.keys(receipt.files ?? {}).length) throw new Error(`Missing artifact manifest: ${name}`);
  if (receipt.target !== 'web') throw new Error(`Missing web build proof: ${name}`);
  targets.push([name, `dist/apps/apps.${name}`, domains[name]]);
}
// Content-hashed build outputs never change under the same name: cache them for a year.
const forever = '  Cache-Control: public, max-age=31536000, immutable\n';
const immutable = { site: `\n/_astro/*\n${forever}`, run: `\n/assets/*\n${forever}`, app: `\n/assets/*\n${forever}` };
for (const [name, source, host] of targets) {
  const dest = join(root, 'dist/publish', name);
  rmSync(dest, {recursive: true, force: true});
  mkdirSync(dest, {recursive: true});
  cpSync(join(root, source), dest, {recursive: true, filter: path => !basename(path).startsWith('.')});
  // Engine host: apps run the headless engine of any tool through it (ADR 0034), so a tool
  // published later works in Flow and Mind without rebuilding them.
  if (name === 'run') {
    const allowed = ['flow', 'mind'].map(app => `https://${domains[app]}`);
    mkdirSync(join(dest, 'engines'), {recursive: true});
    cpSync(join(root, 'web/run-host/host.html'), join(dest, 'engines/host.html'));
    writeFileSync(join(dest, 'engines/host.js'), readFileSync(join(root, 'web/run-host/host.js'), 'utf8').replace('__ALLOWED_ORIGINS__', JSON.stringify(allowed)));
  }
  // Release test artifacts must never become a public distribution feed.
  if (name === 'site' && !features.desktop) {
    for (const file of ['registry', 'install.sh', 'install.ps1']) rmSync(join(dest, file), {recursive: true, force: true});
  }
  let count = 0, bytes = 0;
  const scriptHashes = new Set();
  function inspect(dir) {
    for (const entry of readdirSync(dir, {withFileTypes: true})) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) inspect(path);
      else {
        // The contract and locales of a tool are public data the player reads (ADR 0035); nothing else
        // ending in .toml may ship.
        const toolData = name === 'run' && /^[a-z0-9]+\.[a-z0-9-]+\/(?:tool\.toml|locales\/(?:fr|en)\.toml)$/.test(relative(dest, path).split(sep).join('/'));
        if (entry.isSymbolicLink() || (/(?:\.map|\.rs|\.toml|\.pem|\.env|\.dev\.vars)$/.test(entry.name) && !toolData)) throw new Error(`Non-public artifact: ${path}`);
        if (name === 'site' && entry.name.endsWith('.html')) {
          const html = readFileSync(path, 'utf8');
          for (const [, attributes, body] of html.matchAll(/<script([^>]*)>([\s\S]*?)<\/script>/g)) {
            if (!/\bsrc=|application\/ld\+json/.test(attributes) && body.trim()) scriptHashes.add(`'sha256-${createHash('sha256').update(body).digest('base64')}'`);
          }
        }
        const size = statSync(path).size;
        if (size > 25 * 1024 * 1024) throw new Error(`Asset exceeds the free-plan limit: ${path}`);
        count++; bytes += size;
      }
    }
  }
  inspect(dest);
  // Signed index and contract catalogue: public data the apps fetch without cookies (ADR 0034);
  // their signature, not the transport, is what clients trust.
  const registryHeaders = '\n/registry/*\n  Access-Control-Allow-Origin: *\n  Cache-Control: no-cache\n';
  const csp = name === 'site'
    ? `default-src 'self'; script-src 'self' ${[...scriptHashes].join(' ')}${turnstile}; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' https://${domains.run}${features.account ? ` https://${domains.api}` : ''}; frame-src https://${domains.run}${turnstile}; worker-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'; form-action 'self'`
    : name !== 'run' ? `default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' https://${domains.api} https://${domains.root}; frame-src https://${domains.run}; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'`
    : `default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; worker-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors https://${domains.root} https://${domains.desk} https://${domains.hub} https://${domains.flow} https://${domains.mind}; form-action 'none'`;
  writeFileSync(join(dest, '_headers'), `/*\n  Content-Security-Policy: ${csp}\n  X-Content-Type-Options: nosniff\n  Referrer-Policy: no-referrer\n  Strict-Transport-Security: max-age=31536000\n  Permissions-Policy: camera=(), microphone=(), geolocation=()\n\n/sw.js\n  Cache-Control: no-cache\n\n/*/sw.js\n  Cache-Control: no-cache\n${name === 'site' ? registryHeaders : ''}${immutable[name === 'site' ? 'site' : name === 'run' ? 'run' : 'app']}`);
  if (count > 20000) throw new Error(`Too many assets: ${count}`);
  mkdirSync(join(root, `services/${name}`), {recursive: true});
  writeFileSync(join(root, `services/${name}/wrangler.jsonc`), JSON.stringify({
    name: `kortex-${name}`, compatibility_date: '2026-09-01', workers_dev: false,
    routes: [{pattern: host, custom_domain: true}],
    assets: {directory: `../../dist/publish/${name}`, html_handling: 'auto-trailing-slash', not_found_handling: name === 'site' ? '404-page' : 'none'},
  }, null, 2) + '\n');
  summary[name] = {host, assets: count, bytes};
}
writeFileSync(join(root, 'dist/publish/inventory.json'), JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary, null, 2));
