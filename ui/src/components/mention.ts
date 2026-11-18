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
 * Each `@Name` of one of `names` in `text`, in order, as the service reads a name (`team.rs` `first_named`): case
 * ignored, the longest name at one place, and only where the name ends (`@Al` isn't in `@Alex`).
 */
export function named(text: string, names: readonly string[]): { at: number; length: number }[] {
  const lower = text.toLowerCase();
  const wanted = names.map((n) => `@${n.toLowerCase()}`).sort((a, b) => b.length - a.length);
  const found: { at: number; length: number }[] = [];
  for (let at = lower.indexOf('@'); at >= 0; at = lower.indexOf('@', at + 1)) {
    const hit = wanted.find((m) => lower.startsWith(m, at) && !wordy.test(lower.charAt(at + m.length)));
    if (!hit) continue;
    found.push({ at, length: hit.length });
    at += hit.length - 1;
  }
  return found;
}

/** The one whose `@Name` comes first in `text`: who the service gives a message to, so the window can say so. */
export function firstNamed<T extends { name: string }>(text: string, people: readonly T[]): T | undefined {
  const first = named(
    text,
    people.map((p) => p.name),
  )[0];
  const word = first && text.slice(first.at + 1, first.at + first.length).toLowerCase();
  return people.find((p) => p.name.toLowerCase() === word);
}
