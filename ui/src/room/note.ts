import type { Message, Turn } from '../api';
import { pathKey } from '../components/files';

/** What a part of a design's page is, in a plain word, as the page's notes script names it. */
export const KINDS = ['button', 'link', 'heading', 'text', 'picture', 'field', 'box'] as const;
export type Kind = (typeof KINDS)[number];

/** A part the person clicked on a design's page: its kind, its words and the path that finds it again. */
export interface Part {
  kind: Kind;
  words: string;
  find: string;
}

/** A note's first line, read back: the option's letter, the page's full path and the part. */
export interface Note extends Part {
  letter: string;
  page: string;
}

/** The longest words and path a note carries: all of it comes from a page an agent wrote. */
export const WORDS = 80;
export const FIND = 600;
/**
 * The only form a part's path takes, as the notes script writes it: `body` or an id, then each tag and its place. A
 * page that says anything else is not heard: the path goes into a message sent in the person's name.
 */
export const PATH = /^(body|#[A-Za-z][\w-]*)( > [A-Za-z][\w-]*(:nth-of-type\(\d+\))?)*$/;

/**
 * The one form of a note's first line, which the agent reads and the window reads back:
 * `Design note on option A, C:\…\Designs\Shop\export\a.html: the button "Import orders" at body > main > button`.
 * The person's own words follow on the next lines.
 */
const LINE = new RegExp(
  `^Design note on option ([A-Z]), (.+?): the (${KINDS.join('|')}) "([^"\\n]*)" at ([^\\n]+)$`,
);

/** Words of the page made safe for the line: one line, no double quote, cut short. */
const plain = (s: string, most: number) => s.replace(/\s+/g, ' ').replace(/"/g, "'").trim().slice(0, most);

/** The note's message: its first line, then the person's words. */
export const noteText = (n: Note, words: string) =>
  `Design note on option ${n.letter}, ${n.page}: the ${n.kind} "${plain(n.words, WORDS)}" at ${plain(n.find, FIND)}\n${words}`;

/** A note the chat holds: its message, its number among the notes on its page, and whether it is settled. */
export interface Made {
  id: string;
  note: Note;
  n: number;
  settled: boolean;
}

/**
 * The chat's notes, in the order they were made: each numbered among those on its page, and settled once the turn its
 * message started has ended.
 */
export function notesOf(messages: Message[], turns: Record<string, Turn>): Made[] {
  const counts = new Map<string, number>();
  return messages.flatMap((m) => {
    const read = m.role === 'user' && typeof m.body === 'string' ? readNote(m.body) : undefined;
    if (!read) return [];
    const page = pathKey(read.note.page);
    const n = (counts.get(page) ?? 0) + 1;
    counts.set(page, n);
    const turn = m.turn ? turns[m.turn] : undefined;
    const settled = !!turn && ['done', 'stopped', 'failed'].includes(turn.status);
    return [{ id: m.id, note: read.note, n, settled }];
  });
}

/** A message's note, when it begins with one: the note and the person's words under it. */
export function readNote(text: string): { note: Note; words: string } | undefined {
  const at = text.indexOf('\n');
  const m = LINE.exec(at < 0 ? text : text.slice(0, at));
  if (!m) return undefined;
  const [, letter, page, kind, words, find] = m;
  return { note: { letter, page, kind: kind as Kind, words, find }, words: at < 0 ? '' : text.slice(at + 1) };
}
