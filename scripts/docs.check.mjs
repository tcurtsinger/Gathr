// Fails when the docs stop holding together: a decision without a status or a built line, a built
// claim that names no file, a dead link or heading, a doc nothing links to, a decisions list that is
// out of date, or a rule tag in AGENTS.md that names no promise.
// usage: node scripts/docs.check.mjs            checks
//        node scripts/docs.check.mjs --write    rewrites docs/decisions/README.md first
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, posix } from 'node:path';

const root = join(import.meta.dirname, '..');
const read = (path) => readFileSync(join(root, path), 'utf8').replaceAll('\r\n', '\n');
const tracked = execFileSync('git', ['ls-files', '*.md'], { cwd: root, encoding: 'utf8' })
  .split('\n')
  .filter((path) => path && !path.startsWith('ui/src/registry/'));
const failures = [];

// Decisions: a status, a built line with its proof, and the list of them all.
const LIST = 'docs/decisions/README.md';
const decisions = tracked.filter((path) => /^docs\/decisions\/\d{4}-/.test(path)).sort();
const rows = [];
for (const path of decisions) {
  const lines = read(path).split('\n');
  const number = path.slice('docs/decisions/'.length, 'docs/decisions/'.length + 4);
  const title = lines[0].match(/^# (\d{4})[.:] (.+)$/);
  if (title?.[1] !== number) failures.push(`${path}: its first line isn't "# ${number}. <title>"`);
  const head = lines.slice(0, 12);
  const status = head.find((line) => line.startsWith('Status: '))?.slice(8) ?? '';
  const replaced = status.match(/^replaced by \[(\d{4})\]\(([^)]+)\)/);
  if (!status.startsWith('accepted') && !replaced) {
    failures.push(`${path}: no "Status: accepted" or "Status: replaced by [NNNN](file)" in its first 12 lines`);
  }
  const built = head.find((line) => line.startsWith('Built: '))?.slice(7) ?? '';
  const how = built.match(/^(yes|partly|no)\b/)?.[1];
  if (!how) failures.push(`${path}: no "Built: yes", "Built: partly" or "Built: no" in its first 12 lines`);
  const proof = [...built.matchAll(/`([^`]+)`/g)].map((m) => m[1]).filter((p) => existsSync(join(root, p)));
  if (how && how !== 'no' && !proof.length) {
    failures.push(`${path}: "Built: ${how}" names no file in this repo (a test, in backticks) that proves it`);
  }
  const file = path.slice('docs/decisions/'.length);
  const shown = replaced ? `replaced by ${replaced[1]}` : 'accepted';
  rows.push(`| ${number} | [${title?.[2] ?? file}](${file}) | ${shown} | ${how ?? '?'} |`);
}
const list = `# Decisions

One file per decision: the choice, why, and what was turned down. Each starts with two lines.

- \`Status:\` is \`accepted\`, or \`replaced by\` the decision that took its place. A decision replaced in
  part says which part under its status.
- \`Built:\` is \`yes\`, \`partly\` or \`no\`: whether the code does what the decision says today. \`yes\` and
  \`partly\` name the test that fails if it stops (or, where no test can, the file that is the proof), and
  \`partly\` says what is missing. \`no\` means it is written down only. None of this says the decision has
  held in real use: that is Kept, in [the charter](../charter.md)'s words, and it is counted per promise.

This list is written by \`node scripts/docs.check.mjs --write\` from those lines, and the same script
fails in CI when the two differ.

| # | Decision | Status | Built |
|---|---|---|---|
${rows.join('\n')}
`;
if (process.argv.includes('--write')) writeFileSync(join(root, LIST), list);
if (!existsSync(join(root, LIST)) || read(LIST) !== list) {
  failures.push(`${LIST}: out of date; run node scripts/docs.check.mjs --write`);
}
if (!tracked.includes(LIST) && existsSync(join(root, LIST))) tracked.push(LIST);

// Links: every relative link leads to a file, and its #heading to a heading there.
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
    headings.set(path, new Set([...found].map((m) => slug(m[1].trim()))));
  }
  return headings.get(path);
};
const links = new Map();
let count = 0;
for (const path of tracked) {
  links.set(path, []);
  for (const [, target] of prose(read(path)).matchAll(/\[[^\]]*\]\(([^)\s]+)(?: "[^"]*")?\)/g)) {
    if (/^[a-z][a-z0-9+.-]*:/i.test(target)) continue;
    count += 1;
    const [file, anchor] = target.split('#');
    const to = file ? posix.normalize(posix.join(dirname(path), decodeURI(file))) : path;
    if (to.startsWith('..') || !existsSync(join(root, to))) {
      failures.push(`${path}: the link ${target} leads nowhere`);
      continue;
    }
    links.get(path).push(to);
    const page = statSync(join(root, to)).isFile() && to.endsWith('.md');
    if (anchor && page && !headingsOf(to).has(anchor.toLowerCase())) {
      failures.push(`${path}: the link ${target} names a heading ${to} doesn't have`);
    }
  }
}

// The index: every doc is reached from AGENTS.md or README.md, directly or through another doc.
const reached = new Set();
const walk = (path) => {
  if (reached.has(path)) return;
  reached.add(path);
  for (const to of links.get(path) ?? []) walk(to);
};
walk('AGENTS.md');
walk('README.md');
for (const path of tracked.filter((p) => p.startsWith('docs/'))) {
  if (!reached.has(path)) failures.push(`${path}: nothing links to it from AGENTS.md or README.md`);
}

// AGENTS.md's stopgap rules: each tag names promises the charter has.
const promises = [...read('docs/charter.md').split('## The promises')[1].split('\n## ')[0].matchAll(/^(\d+)\. /gm)];
const tags = [...read('AGENTS.md').matchAll(/ \((P\d+(?:, P\d+)*)\)$/gm)];
for (const [, tag] of tags) {
  for (const p of tag.split(', ')) {
    if (Number(p.slice(1)) < 1 || Number(p.slice(1)) > promises.length) {
      failures.push(`AGENTS.md: the tag (${tag}) names a promise the charter doesn't have`);
    }
  }
}

if (failures.length) {
  console.error(`docs: ${failures.length} to fix:\n${failures.join('\n')}`);
  process.exit(1);
}
console.log(
  `docs: ok, ${decisions.length} decisions, ${count} links in ${tracked.length} files, ` +
    `${tags.length} stopgap rules in AGENTS.md`,
);
