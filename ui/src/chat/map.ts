import type { ThreadMessageLike } from '@assistant-ui/react';
import type { ConversationMapEntry } from '@/components/assistant-ui/elements/conversation-map';
import type { Custom } from './messages';
import { clock } from './team';

/** A message with a place on the conversation map (0039): the person's, or the lead's reply to them. */
export interface Tick {
  /** Its block in the timeline. */
  id: string;
  kind: 'you' | 'lead';
  who: string;
  time: string;
  /** Its first lines, as plain text. */
  text: string;
}

/** A tick as the rail draws it: one message, or a whole earlier day. `from` is its first tick's place. */
export interface MapEntry extends ConversationMapEntry {
  kind: Tick['kind'] | 'day';
  from: number;
}

const LINES = 240;

// Lines with no words of their own: a code fence, a rule or a heading's underline, a table's rule.
const BARE = /^\s*(```|~~~)|^\s*[-*_=][-*_=\s]*$|^\s*\|?[ :|-]*-[ :|-]*\|[ :|-]*$/;
// What opens a line: a heading, a quote, a list item and its checkbox.
const OPENS = /^(\s*(#{1,6}\s+|>\s*|[-*+]\s+|\d+[.)]\s+))+(\[[ xX]\]\s+)?/;

/**
 * A message's first lines as plain text: Markdown's marks go, words stay, code as written. Anything in
 * angle brackets stays too: the chat shows an agent's HTML as text (Markdown.tsx), and `Vec<String>` is words.
 */
export function plain(markdown: string): string {
  const lines = markdown
    .slice(0, 4 * LINES)
    .split(/\r?\n/)
    .filter((line) => !BARE.test(line))
    .map((line) => line.replace(OPENS, ''))
    .join('\n');
  const inline = (text: string) =>
    text
      // A link or an image is its text; its address may hold one pair of brackets.
      .replace(/!?\[([^\]]*)\]\((?:[^()]|\([^()]*\))*\)/g, '$1')
      .replace(/\*+|~~/g, '')
      .replace(/\|/g, ' ')
      // An underscore inside a word is part of a name, not emphasis.
      .replace(/(^|\W)_+/g, '$1')
      .replace(/_+(?!\w)/g, '');
  return lines
    .split(/(`[^`\n]*`)/)
    .map((part, i) => (i % 2 ? part.slice(1, -1) : inline(part)))
    .join('')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, LINES);
}

// A block that didn't change is the same object as on the draw before, so its tick is read once.
const read = new WeakMap<ThreadMessageLike, Tick | null>();

function tickOf(item: ThreadMessageLike): Tick | null {
  const custom = (item.metadata?.custom ?? {}) as Custom;
  const part = typeof item.content === 'string' ? undefined : item.content[0];
  const text = part?.type === 'text' ? part.text : '';
  if (custom.tick === undefined || !text) return null;
  const you = item.role === 'user';
  return {
    id: item.id as string,
    kind: you ? 'you' : 'lead',
    who: you ? 'You' : (custom.agent ?? 'Agent'),
    time: custom.tick,
    text: plain(text),
  };
}

/** The timeline's ticks, in order. */
export function ticksOf(items: readonly ThreadMessageLike[]): Tick[] {
  return items.flatMap((item) => {
    if (!read.has(item)) read.set(item, tickOf(item));
    return read.get(item) ?? [];
  });
}

const DAY = 86_400_000;
const dayStart = (at: Date) => new Date(at.getFullYear(), at.getMonth(), at.getDate()).getTime();
const count = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/**
 * The rail's ticks when it has room for `most` (0039). Every message has its own while they fit. Past
 * that the oldest days fold first, each into one tick that opens its first message; the chat's latest
 * day, today's as a rule, stays. If that day alone is too many, its replies go; past even that, its
 * oldest messages fold into one tick too, so the whole chat stays in reach.
 */
export function mapEntries(ticks: Tick[], most: number, now: Date): MapEntry[] {
  const today = dayStart(now);
  // A message with no time counts as today's.
  const days = ticks.map((t) => {
    const at = new Date(t.time);
    return Number.isNaN(at.getTime()) ? today : dayStart(at);
  });
  const folded = (from: number, end: number, name: string, preview: string): MapEntry => {
    const of = ticks.slice(from, end);
    const yours = of.filter((t) => t.kind === 'you').length;
    const what = yours ? `${count(yours, 'message')} from you` : count(of.length, 'reply', 'replies');
    return { id: of[0].id, kind: 'day', from, title: `${name} · ${what}`, preview };
  };
  const nextDay = (from: number) => {
    const next = days.findIndex((d, i) => i > from && d !== days[from]);
    return next < 0 ? ticks.length : next;
  };
  const entries: MapEntry[] = [];
  let fits = ticks.length;
  // Where the days that stay unfolded begin.
  let open = 0;
  while (fits > most && days[open] < (days.at(-1) ?? 0)) {
    const end = nextDay(open);
    entries.push(folded(open, end, dayName(days[open], today), 'Jumps to the start of that day'));
    fits -= end - open - 1;
    open = end;
  }
  for (const [n, t] of ticks.slice(open).entries()) {
    const title = [t.who, clock(t.time)].filter(Boolean).join(' · ');
    entries.push({ id: t.id, kind: t.kind, from: open + n, title, preview: t.text });
  }
  const kept = entries.length > most ? entries.filter((e) => e.kind !== 'lead') : entries;
  if (kept.length <= most) return kept;
  if (most < 2) return [];
  const rest = kept.slice(kept.length - most + 1);
  return [folded(0, rest[0].from, 'Earlier', 'Jumps to the first of them'), ...rest];
}

/** A weekday within the week, then the date: the same weekday a week on would read as today's. */
function dayName(day: number, today: number): string {
  const at = new Date(day);
  return Math.round((today - day) / DAY) < 7
    ? at.toLocaleDateString([], { weekday: 'long' })
    : at.toLocaleDateString([], { day: 'numeric', month: 'short' });
}
