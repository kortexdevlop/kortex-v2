// Collects the publication journal (docs/validation/*.json) into services/api/src/releases.json,
// which the owner's dashboard shows next to the rollback commands. Run by prepare-publish.mjs.
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const root = resolve(import.meta.dirname, '..');
const dir = resolve(root, 'docs/validation');
const publications = readdirSync(dir)
  .filter((f) => /^\d{4}-\d{2}-\d{2}-.*publication.*\.json$/.test(f))
  .sort()
  .reverse()
  .slice(0, 12)
  .map((file) => {
    const j = JSON.parse(readFileSync(resolve(dir, file), 'utf8'));
    return {
      file,
      date: j.date ?? file.slice(0, 10),
      run: j.release_run ?? j.run ?? null,
      commit: typeof j.commit === 'string' ? j.commit.slice(0, 10) : null,
      versions: j.versions && typeof j.versions === 'object' ? j.versions : {},
    };
  });
writeFileSync(resolve(root, 'services/api/src/releases.json'), JSON.stringify({ publications }, null, 1) + '\n');
console.log(`admin data: ${publications.length} publications`);
