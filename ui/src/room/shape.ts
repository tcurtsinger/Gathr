import type { Message, Question, ToolBody, Turn } from '../api';
import type { RoomState } from './useRoom';

/** A block of words an agent wrote before its last words in a turn: kept in the turn's steps, never its message. */
export interface Note {
  note: string;
}

export type Step = ToolBody | Note;

/** An agent's turn as a card: its last words, its steps, and the replies tucked under it. */
export interface Card {
  kind: 'card';
  id: string;
  turn: Turn;
  /** Its last words: the block streaming now, else its last saved one. */
  text: string;
  /** Its saved messages of words, notes too: thumbs-ups on any show, and a search hit on any opens the card. */
  said: Message[];
  /** The message of its last words: the person's thumbs-up goes on it. */
  last?: Message;
  /** Its tool runs and its working notes, in the order they came. */
  steps: Step[];
  at: number;
  replies: Item[];
}

/** The person's message. */
export interface Said {
  kind: 'said';
  id: string;
  message: Message;
  at: number;
  replies: Item[];
}

/** An agent's question to the person: a card of its own, at the time it was asked. */
export interface Asked {
  kind: 'question';
  id: string;
  question: Question;
  at: number;
}

/** The quiet centred line a compaction left: "Compacted Ann's conversation: 468K to 52K". */
export interface Line {
  kind: 'line';
  id: string;
  text: string;
  at: number;
}

export type Item = Card | Said;

const time = (iso?: string | null) => (iso ? Date.parse(iso) : Number.NaN);
const open = (t: Turn) => t.status === 'running' || t.status === 'queued';

/**
 * The room as the screen shows it: the person's messages and each agent's main message in the main chat, by when
 * they started; a reply, the service's or the person's, under the main message it answers. Of Gathr's own lines only
 * a compaction's shows, as a quiet line (the line that opens an answered question's turn isn't: the question's card
 * shows the answer), and a turn that said nothing and ended shows nothing, nor does a compaction's turn. An agent's
 * question to the person sits in the main chat when it was asked.
 */
export function shape(s: RoomState): (Item | Asked | Line)[] {
  const byTurn = new Map<string, Message[]>();
  // The person's message that opened a turn: the service makes the turn just before it, and its card goes after it.
  const opener = new Map<string, number>();
  for (const m of s.messages) {
    if (m.role === 'user' && m.turn) opener.set(m.turn, time(m.time));
    if (m.role !== 'assistant' || !m.turn) continue;
    byTurn.set(m.turn, [...(byTurn.get(m.turn) ?? []), m]);
  }
  const cards = new Map<string, Card>();
  for (const id of new Set([...Object.keys(s.turns), ...byTurn.keys()])) {
    const mine = byTurn.get(id) ?? [];
    // Words whose turn the history doesn't have, as some old chats' are, still show.
    const turn = s.turns[id] ?? {
      id,
      agent: null,
      status: 'done',
      why: null,
      ref: '',
      created: mine[0]?.time,
    };
    const streaming = s.partial[turn.id];
    if ((turn.status === 'queued' && !streaming) || turn.opened === 'compaction') continue;
    const said = mine.filter((m) => m.kind === 'text' && typeof m.body === 'string');
    // Only the last words are the message. While the turn runs, words it went on from to a tool are a note already.
    const newest = said.at(-1);
    const moved = mine.findLast((m) => m.kind === 'tool' || m.kind === 'text') !== newest;
    const last = !streaming && newest && !(open(turn) && moved) ? newest : undefined;
    const text = streaming || ((last?.body as string | undefined) ?? '');
    if (!text && turn.status === 'done') continue;
    const steps = mine.flatMap((m): Step[] => {
      if (m.kind === 'tool') return [m.body as ToolBody];
      return m.kind === 'text' && m !== last && m.body ? [{ note: m.body as string }] : [];
    });
    const at = Math.max(time(turn.created ?? mine[0]?.time), opener.get(turn.id) ?? 0);
    cards.set(turn.id, { kind: 'card', id: turn.id, turn, text, said, last, steps, at, replies: [] });
  }
  const cardOf = new Map<string, string>();
  for (const c of cards.values()) for (const m of byTurn.get(c.id) ?? []) cardOf.set(m.id, c.id);

  // Where each item sits: under the main message its `reply_to` names (an agent's card or the person's message),
  // or in the main chat. An agent's card sits where its last words do.
  const parent = new Map<string, string>();
  const under = (id: string, to?: string | null) => {
    const host = to && (cardOf.get(to) ?? to);
    if (host && host !== id) parent.set(id, host);
  };
  for (const c of cards.values()) under(c.id, s.partial[c.id] ? s.under[c.id] : c.said.at(-1)?.reply_to);
  const items: Item[] = [...cards.values()];
  for (const m of s.messages) {
    if (m.role !== 'user' || m.kind !== 'text') continue;
    items.push({ kind: 'said', id: m.id, message: m, at: time(m.time), replies: [] });
    under(m.id, m.reply_to);
  }

  // Replies are flat under the main message they belong to.
  const root = (id: string) => {
    const seen = new Set<string>();
    let at = id;
    while (parent.has(at) && !seen.has(at)) {
      seen.add(at);
      at = parent.get(at) as string;
    }
    return at;
  };
  // At the same moment, the person's message comes before the card that answers it.
  const byAt = (a: Item, b: Item) =>
    (a.at || 0) - (b.at || 0) || Number(a.kind === 'card') - Number(b.kind === 'card');
  const byId = new Map(items.map((i) => [i.id, i]));
  const main: Item[] = [];
  for (const item of items.sort(byAt)) {
    const top = parent.has(item.id) ? byId.get(root(item.id)) : undefined;
    if (top && top !== item) top.replies.push(item);
    else main.push(item);
  }
  const asked = s.questions.map(
    (q): Asked => ({ kind: 'question', id: `q${q.id}`, question: q, at: time(q.asked) }),
  );
  const lines = s.messages.flatMap((m): Line[] =>
    m.compacted ? [{ kind: 'line', id: m.id, text: m.body as string, at: time(m.time) }] : [],
  );
  return [...main, ...asked, ...lines].sort((a, b) => (a.at || 0) - (b.at || 0));
}

/** A note the turn kept in Memory: what `note` answered, "Noted about you:" then the note. */
export interface Noted {
  label: string;
  text: string;
}

/** The notes the turn's `note` calls kept, in the order they came: the step keeps each answer. */
export function notedOf(steps: Step[]): Noted[] {
  return steps.flatMap((s): Noted[] => {
    if ('note' in s || s.tool !== 'mcp__gathr__note' || s.status !== 'ok' || !s.result) return [];
    const [label, ...text] = s.result.text.split('\n');
    return label.endsWith(':') && text.length ? [{ label: label.slice(0, -1), text: text.join(' ') }] : [];
  });
}

/** Whether a card's turn is still under way. */
export const working = (c: Card) => open(c.turn);

const COUNTED: Record<string, [string, string]> = {
  Read: ['Read', 'file'],
  Edit: ['Edited', 'file'],
  Write: ['Edited', 'file'],
  Bash: ['Ran', 'command'],
  Shell: ['Ran', 'command'],
  PowerShell: ['Ran', 'command'],
};

/**
 * What a turn's steps did, as its one line: "Read 4 files, ran 2 commands", "Ran 14 steps", "5 steps so far". A
 * working note counts as a step.
 */
export function stepsLine(steps: Step[], running: boolean): string {
  const n = steps.length;
  if (running) return `${n} ${n === 1 ? 'step' : 'steps'} so far`;
  const counts = new Map<string, number>();
  let other = 0;
  for (const s of steps) {
    const kind = 'tool' in s ? COUNTED[s.tool] : undefined;
    if (!kind) other++;
    else counts.set(kind.join(' '), (counts.get(kind.join(' ')) ?? 0) + 1);
  }
  if (counts.size === 0) return `Ran ${n} ${n === 1 ? 'step' : 'steps'}`;
  const parts = [...counts].map(([k, c]) => {
    const [verb, noun] = k.split(' ');
    return `${verb.toLowerCase()} ${c} ${noun}${c === 1 ? '' : 's'}`;
  });
  if (other) parts.push(`${other} more ${other === 1 ? 'step' : 'steps'}`);
  const line = parts.join(', ');
  return line.charAt(0).toUpperCase() + line.slice(1);
}
