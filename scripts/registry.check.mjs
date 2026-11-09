// Fails if a file in ui/src/registry differs from the upstream registry file ui/registry.json records,
// or isn't recorded at all. Those files stay as published (0030); a needed change is an adaptation
// recorded with its new hash and why. usage: node scripts/registry.check.mjs
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';

const root = join(import.meta.dirname, '..', 'ui');
const folder = join(root, 'src', 'registry');
const { files } = JSON.parse(readFileSync(join(root, 'registry.json'), 'utf8'));
const recorded = new Map(files.map((f) => [f.path, f]));

const failures = [];
for (const entry of readdirSync(folder, { recursive: true, withFileTypes: true })) {
  if (!entry.isFile()) continue;
  const path = relative(folder, join(entry.parentPath, entry.name)).replaceAll('\\', '/');
  const file = recorded.get(path);
  recorded.delete(path);
  if (!file) {
    failures.push(`${path}: not in ui/registry.json`);
    continue;
  }
  const hash = createHash('sha256').update(readFileSync(join(folder, path))).digest('hex');
  const expected = file.adapted ?? file.sha256;
  if (hash !== expected) failures.push(`${path}: sha256 ${hash}, expected ${expected}`);
}
for (const path of recorded.keys()) failures.push(`${path}: in ui/registry.json but missing`);

if (failures.length) {
  console.error(`registry: files differ from what ui/registry.json records:\n${failures.join('\n')}`);
  process.exit(1);
}
console.log(`registry: ok, ${files.length} files`);
