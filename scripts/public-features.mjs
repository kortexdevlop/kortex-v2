import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
export function publicFeatures(root) {
  const text = readFileSync(resolve(root, 'brand/features.toml'), 'utf8');
  const enabled = (name, variable) => process.env[variable] === undefined
    ? new RegExp(`^${name}\\s*=\\s*true\\s*$`, 'm').test(text)
    : process.env[variable] === 'true';
  return {account:enabled('account','PUBLIC_ACCOUNT_ENABLED'), desktop:enabled('desktop_releases','PUBLIC_DESKTOP_RELEASES_ENABLED')};
}
