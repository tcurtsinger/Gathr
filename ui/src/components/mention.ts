/** The "@" being typed just before the caret, at the start of a word: where it is and what follows it. */
export function typing(text: string, caret: number): { at: number; query: string } | null {
  const m = /(^|\s)@([^\s@]*)$/.exec(text.slice(0, caret));
  return m ? { at: caret - m[2].length - 1, query: m[2].toLowerCase() } : null;
}

/** The text with the "@" typed at `at` (and the rest of its word) replaced by `@Name `, and where the caret goes. */
export function insert(text: string, at: number, caret: number, name: string) {
  const before = `${text.slice(0, at)}@${name} `;
  const after = text.slice(caret).replace(/^[^\s@]*\s*/, '');
  return { text: before + after, caret: before.length };
}

const wordy = /[\p{L}\p{N}_]/u;

/**
 * The one whose `@Name` comes first in `text`, ignoring case; at one place, the longest name. A name counts only
 * where it ends: `@Al` isn't in `@Alex`. The service's own rule (`team.rs` `first_named`), so the window says what
 * the service will do.
 */
export function firstNamed<T extends { name: string }>(text: string, people: readonly T[]): T | undefined {
  const lower = text.toLowerCase();
  let best: { at: number; len: number; who: T } | undefined;
  for (const who of people) {
    const mention = `@${who.name.toLowerCase()}`;
    let at = lower.indexOf(mention);
    while (at >= 0 && wordy.test(lower.charAt(at + mention.length))) at = lower.indexOf(mention, at + 1);
    if (at < 0) continue;
    if (!best || at < best.at || (at === best.at && mention.length > best.len))
      best = { at, len: mention.length, who };
  }
  return best?.who;
}

/** The text cut where each `@Name` of someone named stands, as the service reads a name (see `firstNamed`). */
export function marks(text: string, names: readonly string[]): { at: number; text: string; name: boolean }[] {
  const lower = text.toLowerCase();
  const wanted = names.map((n) => `@${n.toLowerCase()}`).sort((a, b) => b.length - a.length);
  const parts: { at: number; text: string; name: boolean }[] = [];
  let from = 0;
  for (let at = lower.indexOf('@'); at >= 0; at = lower.indexOf('@', at + 1)) {
    if (at < from) continue;
    const hit = wanted.find((m) => lower.startsWith(m, at) && !wordy.test(lower.charAt(at + m.length)));
    if (!hit) continue;
    if (at > from) parts.push({ at: from, text: text.slice(from, at), name: false });
    parts.push({ at, text: text.slice(at, at + hit.length), name: true });
    from = at + hit.length;
  }
  if (from < text.length) parts.push({ at: from, text: text.slice(from), name: false });
  return parts;
}
