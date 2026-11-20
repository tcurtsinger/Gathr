// Fails when a link in a Markdown file leads nowhere, or names a heading its page doesn't have.
// docs/archive is history and isn't checked.
// usage: node scripts/docs.check.mjs
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, posix } from 'node:path';

const root = join(import.meta.dirname, '..');
const read = (path) => readFileSync(join(root, path), 'utf8').replaceAll('\r\n', '\n');
const tracked = execFileSync('git', ['ls-files', '*.md'], { cwd: root, encoding: 'utf8' })
  .split('\n')
  .filter((path) => path && !path.startsWith('ui/src/registry/') && !path.startsWith('docs/archive/'));
const failures = [];

const prose = (text) => text.replace(/^(```|~~~).*?^\1.*?$/gms, '').replace(/`[^`\n]*`/g, '');
const slug = (heading) =>
  heading
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, '')
    .replaceAll(' ', '-');
const headings = new Map();
const headingsOf = (path) => {
  if (!headings.has(path)) {
    const found = read(path).replace(/^(```|~~~).*?^\1.*?$/gms, '').matchAll(/^#{1,6} (.+)$/gm);
    // A repeated heading's later anchors end -1, -2, as GitHub makes them.
    const anchors = new Set();
    for (const [, heading] of found) {
      const base = slug(heading.trim());
      let anchor = base;
      for (let nth = 1; anchors.has(anchor); nth += 1) anchor = `${base}-${nth}`;
      anchors.add(anchor);
    }
    headings.set(path, anchors);
  }
  return headings.get(path);
};
let count = 0;
for (const path of tracked) {
  // Inline links, with a title in any of its three forms, and reference links by their definitions.
  const text = prose(read(path));
  const inline = text.matchAll(/\[[^\]]*\]\(([^)\s]+)(?:\s+(?:"[^"]*"|'[^']*'|\([^)]*\)))?\)/g);
  const defined = text.matchAll(/^ {0,3}\[[^\]^][^\]]*\]:\s*<?([^\s>]+)/gm);
  for (const [, target] of [...inline, ...defined]) {
    if (/^[a-z][a-z0-9+.-]*:/i.test(target)) continue;
    count += 1;
    const [file, anchor] = target.split('#');
    const to = file ? posix.normalize(posix.join(dirname(path), decodeURI(file))) : path;
    if (to.startsWith('..') || !existsSync(join(root, to))) {
      failures.push(`${path}: the link ${target} leads nowhere`);
      continue;
    }
    const page = statSync(join(root, to)).isFile() && to.endsWith('.md');
    if (anchor && page && !to.startsWith('docs/archive/') && !headingsOf(to).has(anchor.toLowerCase())) {
      failures.push(`${path}: the link ${target} names a heading ${to} doesn't have`);
    }
  }
}

if (failures.length) {
  console.error(`docs: ${failures.length} to fix:\n${failures.join('\n')}`);
  process.exit(1);
}
console.log(`docs: ok, ${count} links in ${tracked.length} files`);
