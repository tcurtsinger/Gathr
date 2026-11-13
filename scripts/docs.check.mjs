// Fails when the docs stop holding together: a decision without a status or a built line, a built
// claim that names no file, a dead link or heading, a doc nothing links to, a decisions list that is
// out of date, a rule tag in AGENTS.md that names no promise, or a part of the truth map that says
// built and names a file or a test that isn't there.
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
const isFile = (path) => existsSync(join(root, path)) && statSync(join(root, path)).isFile();

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
  const proof = [...built.matchAll(/`([^`]+)`/g)].map((m) => m[1]).filter(isFile);
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
const links = new Map();
let count = 0;
for (const path of tracked) {
  links.set(path, []);
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
const numbered = read('docs/charter.md').split('## The promises')[1].split('\n## ')[0].matchAll(/^(\d+)\. /gm);
const promises = new Set([...numbered].map((m) => `P${m[1]}`));
const tags = [...read('AGENTS.md').matchAll(/ \((P\d+(?:, P\d+)*)\)$/gm)];
for (const [, tag] of tags) {
  for (const p of tag.split(', ')) {
    if (!promises.has(p)) {
      failures.push(`AGENTS.md: the tag (${tag}) names a promise the charter doesn't have`);
    }
  }
}

// The truth map: each part of each promise is Written, Built or Kept, and what is built names a
// file that exists and a test that is in the file it names.
const MAP = 'docs/truth-map.md';
const states = { Written: 0, Partly: 0, Built: 0, Kept: 0 };
const mapped = new Set();
for (const row of read(MAP).matchAll(/^\| (\d+)\.\d+ \|.*$/gm)) {
  // Cells are split on the pipes Markdown splits on: an escaped one stays in its cell.
  const cells = row[0].trim().slice(1, -1).split(/(?<!\\)\|/).map((cell) => cell.trim());
  const [part, , state, code = '', test = ''] = cells;
  const at = `${MAP}: part ${part}`;
  mapped.add(`P${row[1]}`);
  if (cells.length !== 6) failures.push(`${at} has ${cells.length} cells, not 6`);
  if (!promises.has(`P${row[1]}`)) failures.push(`${at} is under a promise the charter doesn't have`);
  if (!Object.hasOwn(states, state)) {
    failures.push(`${at}: its state is "${state}", not Written, Partly, Built or Kept`);
    continue;
  }
  states[state] += 1;
  const named = (cell) => [...cell.matchAll(/`([^`]+)`/g)].map((m) => m[1]);
  // In the Code cell a name with a slash or a source file's ending is a file; the rest are what's in it.
  const files = named(code).filter((name) => /[\\/]|\.(rs|tsx?|[cm]?js|css|sql|md|json|toml|ya?ml)$/.test(name));
  for (const file of files) if (!isFile(file)) failures.push(`${at}: its code names ${file}, which isn't a file`);
  const proofs = named(test).map((name) => name.split('::'));
  for (const [file, name] of proofs) {
    if (!name) failures.push(`${at}: its test \`${file}\` isn't written as file::test name`);
    else if (!isFile(file)) failures.push(`${at}: its test names ${file}, which isn't a file`);
    else {
      // The whole name: a Rust test's function, or a UI test's title between its quotes.
      const text = read(file).replaceAll("\\'", "'").replaceAll('\\"', '"');
      const whole = file.endsWith('.rs')
        ? text.includes(`fn ${name}(`)
        : ["'", '"', '`'].some((quote) => text.includes(`${quote}${name}${quote}`));
      if (!whole) failures.push(`${at}: ${file} has no test named "${name}"`);
    }
  }
  if ([...files, ...proofs.map(([file]) => file)].some((file) => file.includes('\\'))) {
    failures.push(`${at}: write its paths with /, as the repo does`);
  }
  if (state === 'Written') continue;
  if (!files.length) failures.push(`${at} is ${state} but names no file that does it`);
  // A part with code and no test is Partly at most, and its last cell says so.
  if (state !== 'Partly' && !proofs.length) failures.push(`${at} is ${state} but names no test that fails if it stops`);
}
for (const p of promises) if (!mapped.has(p)) failures.push(`${MAP}: no part for promise ${p.slice(1)}`);

if (failures.length) {
  console.error(`docs: ${failures.length} to fix:\n${failures.join('\n')}`);
  process.exit(1);
}
const parts = Object.entries(states).map(([state, n]) => `${n} ${state}`);
console.log(
  `docs: ok, ${decisions.length} decisions, ${count} links in ${tracked.length} files, ` +
    `${tags.length} stopgap rules in AGENTS.md; the truth map: ${parts.join(', ')}`,
);
