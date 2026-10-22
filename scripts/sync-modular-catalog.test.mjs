import { test } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, sign, createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { syncModularCatalog } from './sync-modular-catalog.mjs';

test('publishes only signed, hash-bound catalogue members alongside legacy assets', () => {
  const root = mkdtempSync(join(tmpdir(), 'kortex-modular-'));
  try {
    const source = join(root, 'source'), output = join(root, 'output');
    mkdirSync(join(source, 'contracts'), { recursive: true });
    const { publicKey, privateKey } = generateKeyPairSync('ed25519');
    const key = publicKey.export({ format: 'der', type: 'spki' }).subarray(-32).toString('base64');
    const contract = { schema: 1, tool: { id: 'test.one', version: '1.0.0' } };
    const fragment = Buffer.from(JSON.stringify({ schema: 1, format: 2, kind: 'catalog-contract', id: 'test.one', version: '1.0.0', contract }));
    const hash = createHash('sha256').update(fragment).digest('hex');
    const path = `contracts/${hash}.json`;
    const index = { schema: 1, format: 2, kind: 'catalog-index', channel: 'stable', sequence: 1, tools: { 'test.one': { path, sha256: hash, bytes: fragment.length, version: '1.0.0' } } };
    const publishRoot = () => {
      const bytes = Buffer.from(JSON.stringify(index));
      writeFileSync(join(source, 'catalog-index-v2.json'), bytes);
      writeFileSync(join(source, 'catalog-index-v2.json.sig'), sign(null, bytes, privateKey).toString('base64'));
    };
    writeFileSync(join(source, path), fragment);
    writeFileSync(join(source, 'contracts/unreferenced.json'), 'never published');
    publishRoot();
    syncModularCatalog(source, output, key);
    assert.deepEqual(readFileSync(join(output, path)), fragment);
    assert.equal(existsSync(join(output, 'contracts/unreferenced.json')), false);
    writeFileSync(join(source, path), 'corrupt');
    assert.throws(() => syncModularCatalog(source, output, key), /digest mismatch/);
    writeFileSync(join(source, path), fragment);
    index.tools['test.one'].path = '../secret'; publishRoot();
    assert.throws(() => syncModularCatalog(source, output, key), /Unsafe/);
    index.tools['test.one'].path = path; index.channel = 'test'; publishRoot();
    assert.throws(() => syncModularCatalog(source, output, key), /Unsupported/);
    index.channel = 'stable'; publishRoot();
    writeFileSync(join(source, 'catalog-index-v2.json.sig'), Buffer.alloc(64).toString('base64'));
    assert.throws(() => syncModularCatalog(source, output, key), /Untrusted/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
