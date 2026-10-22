import { createHash, createPublicKey, verify } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync, realpathSync, statSync } from 'node:fs';
import { join, relative, isAbsolute } from 'node:path';

// Publication-time copy only. Runtime consumers still verify the root and selected fragment.
export function syncModularCatalog(source, destination, publicKey) {
  const read = (path, limit) => {
    if (statSync(path).size > limit) throw new Error('Catalogue resource too large');
    return readFileSync(path);
  };
  const bytes = read(join(source, 'catalog-index-v2.json'), 4 * 1024 * 1024);
  const signature = read(join(source, 'catalog-index-v2.json.sig'), 256);
  const key = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), Buffer.from(publicKey.trim(), 'base64')]), format: 'der', type: 'spki' });
  if (!verify(null, bytes, key, Buffer.from(signature.toString().trim(), 'base64'))) throw new Error('Untrusted modular catalogue');
  const index = JSON.parse(bytes);
  if (index.schema !== 1 || index.format !== 2 || index.kind !== 'catalog-index' || index.channel !== 'stable' || !Number.isSafeInteger(index.sequence) || index.sequence < 1 || !index.tools || Array.isArray(index.tools) || Object.keys(index.tools).length > 10000) throw new Error('Unsupported modular catalogue');
  const resolvedRoot = realpathSync(source);
  mkdirSync(join(destination, 'contracts'), { recursive: true });
  for (const [id, descriptor] of Object.entries(index.tools)) {
    if (!/^[a-z0-9-]+\.[a-z0-9-]+$/.test(id) || !/^[a-f0-9]{64}$/.test(descriptor.sha256) || descriptor.path !== `contracts/${descriptor.sha256}.json` || !Number.isSafeInteger(descriptor.bytes) || descriptor.bytes <= 0 || descriptor.bytes > 4 * 1024 * 1024) throw new Error('Unsafe catalogue descriptor');
    const path = realpathSync(join(source, descriptor.path));
    const local = relative(resolvedRoot, path);
    if (local.startsWith('..') || isAbsolute(local)) throw new Error('Catalogue fragment escapes root');
    const fragment = read(path, descriptor.bytes);
    if (fragment.length !== descriptor.bytes || createHash('sha256').update(fragment).digest('hex') !== descriptor.sha256) throw new Error('Catalogue fragment digest mismatch');
    const document = JSON.parse(fragment);
    if (document.schema !== 1 || document.format !== 2 || document.kind !== 'catalog-contract' || document.id !== id || document.version !== descriptor.version || document.contract.schema !== 1 || document.contract.tool.id !== id || document.contract.tool.version !== descriptor.version) throw new Error('Catalogue fragment identity mismatch');
    writeFileSync(join(destination, descriptor.path), fragment);
  }
  // Publish the signed root last, after validating and copying all immutable members.
  writeFileSync(join(destination, 'catalog-index-v2.json.sig'), signature);
  writeFileSync(join(destination, 'catalog-index-v2.json'), bytes);
}
