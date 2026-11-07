// Makes a public test recording (crates/gathr/tests/frames) from a chat's raw engine frames.
// Keeps the frames and their order; drops what belongs to the person: their folders and user
// name, their account's synced plugins, skills, sub-agents and hook output, and their plan usage.
//
// usage: node scripts/scrub-frames.mjs <export.jsonl> <out.jsonl> <data folder>
// The export is one {"dir", "line"} per row, from:
//   gathr diag query "SELECT direction AS dir, body AS line FROM engine_raw WHERE chat_id = '<chat>' ORDER BY id"
// scripts/scrub-frames.check.mjs checks it (CI runs it).
import fs from 'node:fs';
import os from 'node:os';
import { fileURLToPath } from 'node:url';

const BUILTIN_AGENTS = ['claude', 'Explore', 'general-purpose', 'Plan', 'statusline-setup'];

export const scrub = (f) => {
  if (f?.subtype === 'init') {
    // Named entries become placeholders, so a test still sees how many got through.
    let n = 0;
    const hide = (x) => (typeof x === 'string' ? `removed-${++n}` : { name: `removed-${++n}` });
    const builtin = (x) => typeof x === 'object' && String(x.source ?? '').endsWith('@builtin');
    for (const k of ['plugins', 'skills', 'slash_commands', 'terminal_slash_commands']) {
      if (Array.isArray(f[k])) f[k] = f[k].map((x) => (builtin(x) ? x : hide(x)));
    }
    // Sub-agents come as bare names, with no source: keep only those a signed-out, empty folder lists.
    if (Array.isArray(f.agents)) f.agents = f.agents.map((x) => (BUILTIN_AGENTS.includes(x) ? x : hide(x)));
  }
  if (f?.subtype?.startsWith?.('hook_')) for (const k of ['output', 'stdout', 'stderr']) if (k in f) f[k] = '';
  if (f?.type === 'rate_limit_event') f.rate_limit_info = { status: 'allowed', rateLimitType: 'five_hour' };
  if (f && 'credential_expires_at' in f) f.credential_expires_at = '2030-01-01T00:00:00.000Z';
  return f;
};

const main = (src, dst, data) => {
  if (!src || !dst || !data) throw new Error('usage: scrub-frames.mjs <export.jsonl> <out.jsonl> <data folder>');
  const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const forms = (p) => [p, p.replaceAll('\\', '/'), p.replace(/[:\\/]/g, '-')];
  const swaps = [
    ...forms(data).map((f) => [new RegExp(escape(f), 'gi'), 'gathr-test-data']),
    ...forms(os.homedir()).map((f) => [new RegExp(escape(f), 'gi'), 'home']),
    [new RegExp(escape(os.userInfo().username), 'gi'), 'someone'],
    [/cc-msg-[0-9a-f]+/g, 'cc-msg-test'],
  ];
  const clean = (v) => {
    if (typeof v === 'string') return swaps.reduce((s, [re, to]) => s.replace(re, to), v);
    if (Array.isArray(v)) return v.map(clean);
    if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, clean(x)]));
    return v;
  };

  const rows = [];
  const text = fs.readFileSync(src, 'utf8');
  const exported = text.trimStart().startsWith('{"rows"') ? JSON.parse(text).rows : text.split('\n').filter(Boolean).map((l) => JSON.parse(l));
  for (const row of exported.filter((r) => ['in', 'out', 'err'].includes(r.dir))) {
    let line = row.line;
    try {
      line = JSON.stringify(clean(scrub(JSON.parse(line))));
    } catch {
      line = clean(line);
    }
    rows.push(JSON.stringify({ dir: row.dir, line }));
  }
  fs.writeFileSync(dst, rows.join('\n') + '\n');
  console.log(`${rows.length} frames written to ${dst}. Read it for anything personal before committing it.`);
};

if (process.argv[1] === fileURLToPath(import.meta.url)) main(...process.argv.slice(2));
