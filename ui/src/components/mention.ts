import type { PickedName } from '../api';

/** A name picked from the "@" list: where its `@Name` starts in the text, and whose it is. The service adds names typed whole. */
export interface Picked {
  at: number;
  id: string;
  name: string;
}

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

/**
 * Where `old` became `next`: the place, how much went and how much came. The caret settles a typed or deleted run
 * (`aa` to `aaa` could be either letter); anything else is the span between what the two share at each end.
 */
function changed(old: string, next: string, caret: number) {
  const d = next.length - old.length;
  if (d > 0 && caret >= d && next.slice(0, caret - d) + next.slice(caret) === old)
    return { at: caret - d, gone: 0, came: d };
  if (d < 0 && old.slice(0, caret) + old.slice(caret - d) === next) return { at: caret, gone: -d, came: 0 };
  let at = 0;
  while (at < old.length && at < next.length && old[at] === next[at]) at++;
  let end = 0;
  while (end < old.length - at && end < next.length - at && old.at(-1 - end) === next.at(-1 - end)) end++;
  return { at, gone: old.length - at - end, came: next.length - at - end };
}

const wordy = /[\p{L}\p{N}_]/u;

/** Whether `@Name` still stands whole at the pick's place: the same letters, not run on into a longer word. */
const stands = (text: string, p: Picked) => {
  const end = p.at + p.name.length + 1;
  return text.slice(p.at, end) === `@${p.name}` && !wordy.test(text.charAt(end));
};

/**
 * The picks once `old` became `next` (the caret where it is now): those after the change move with it, those before
 * stay, and one the change touched inside, or that no longer stands whole, is plain text from then on.
 */
export function follow(old: string, next: string, caret: number, picks: readonly Picked[]): Picked[] {
  if (!picks.length) return [];
  const c = changed(old, next, caret);
  const kept: Picked[] = [];
  for (const p of picks) {
    const end = p.at + p.name.length + 1;
    if (c.at + c.gone <= p.at) kept.push({ ...p, at: p.at + c.came - c.gone });
    else if (c.at >= end) kept.push(p);
  }
  return kept.filter((p) => stands(next, p));
}

/** The pick that gets the message: the first in the text. */
export const first = (picks: readonly Picked[]) => [...picks].sort((a, b) => a.at - b.at)[0];

/** The picks as the message keeps them: each `@Name`'s place and length, and whose it is. */
export const kept = (picks: readonly Picked[]): PickedName[] =>
  picks.map((p) => ({ at: p.at, len: p.name.length + 1, agent: p.id }));
