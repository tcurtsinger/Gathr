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
