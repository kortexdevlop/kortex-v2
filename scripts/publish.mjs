// Publish already-prepared static assets; credentials stay in Wrangler's environment.
import { publicFeatures } from './public-features.mjs';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
const root = resolve(import.meta.dirname, '..');
const onlySite = process.argv.includes('--site-only');
const onlyStatic = process.argv.includes('--static-only');
if (process.argv.slice(2).some(arg => !['--site-only', '--static-only'].includes(arg))) throw new Error('Unknown publication option');
const inventory = JSON.parse(readFileSync(resolve(root, 'dist/publish/inventory.json'), 'utf8'));
function run(args) {
  const result = spawnSync('wrangler', args, {cwd: root, stdio: 'inherit', env: {...process.env, WRANGLER_SEND_METRICS: 'false'}});
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
if (publicFeatures(root).account) {
  if (!onlySite && !onlyStatic) {
  run(['d1','migrations','apply','DB','--remote','--env','production','--config','services/api/wrangler.jsonc']);
  run(['deploy','--env','production','--config','services/api/wrangler.jsonc']);
  }
  const domains = readFileSync(resolve(root, 'brand/domains.toml'), 'utf8');
  const host = /^api\s*=\s*"([^"]+)"/m.exec(domains)?.[1];
  const health = await fetch(`https://${host}/v1/health`);
  if (!health.ok || (await health.json()).ok !== true) throw new Error('Account API must be healthy before publishing its UI');
}
console.log('Publishing static assets:', JSON.stringify(inventory));
for (const name of (onlySite ? ['site'] : ['run', ...Object.keys(inventory).filter(name => !['run', 'site'].includes(name)), 'site'])) run(['deploy', '--config', `services/${name}/wrangler.jsonc`]);
