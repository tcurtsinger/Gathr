import type { Message, ToolBody, Turn } from '../api';
import type { RoomState } from './useRoom';

/** An agent's turn as a card: its words, its tool runs, and the replies tucked under it. */
export interface Card {
  kind: 'card';
  id: string;
  turn: Turn;
  /** Its saved words, then the words still streaming. */
  text: string;
  /** Its saved messages of words: thumbs-ups go on these. */
  said: Message[];
  steps: ToolBody[];
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

export type Item = Card | Said;

const time = (iso?: string | null) => (iso ? Date.parse(iso) : Number.NaN);
const open = (t: Turn) => t.status === 'running' || t.status === 'queued';

/**
 * The room as the screen shows it: the person's messages and each agent's main message in the main chat, by when
 * they started; an agent's reply to another agent's message, and the person's reply to an agent's, under it. Gathr's
 * own lines aren't shown, and a turn that said nothing and ended shows nothing.
 */
export function shape(s: RoomState): Item[] {
  const byTurn = new Map<string, Message[]>();
  for (const m of s.messages) {
    if (m.role !== 'assistant' || !m.turn) continue;
    byTurn.set(m.turn, [...(byTurn.get(m.turn) ?? []), m]);
  }
  const cards = new Map<string, Card>();
  for (const turn of Object.values(s.turns)) {
    if (!turn.agent || (turn.status === 'queued' && !s.partial[turn.id])) continue;
    const mine = byTurn.get(turn.id) ?? [];
    const said = mine.filter((m) => m.kind === 'text' && typeof m.body === 'string');
    const text = [...said.map((m) => m.body as string), s.partial[turn.id] ?? '']
      .filter(Boolean)
      .join('\n\n');
    if (!text && turn.status === 'done') continue;
    const steps = mine.filter((m) => m.kind === 'tool').map((m) => m.body as ToolBody);
    const at = time(turn.created ?? mine[0]?.time);
    cards.set(turn.id, { kind: 'card', id: turn.id, turn, text, said, steps, at, replies: [] });
  }
  const cardOf = new Map<string, string>();
  for (const c of cards.values()) for (const m of byTurn.get(c.id) ?? []) cardOf.set(m.id, c.id);

  // Where each item sits: under another agent's card, or in the main chat.
  const parent = new Map<string, string>();
  for (const c of cards.values()) {
    const topic = c.turn.topic?.startsWith('b:') ? c.turn.topic.slice(2) : undefined;
    const reply = c.said.find((m) => m.reply_to)?.reply_to;
    const under = topic && topic !== c.id ? topic : reply && cardOf.get(reply);
    if (under && cards.has(under)) parent.set(c.id, under);
  }
  const items: Item[] = [...cards.values()];
  for (const m of s.messages) {
    if (m.role !== 'user' || m.kind !== 'text') continue;
    items.push({ kind: 'said', id: m.id, message: m, at: time(m.time), replies: [] });
    const host = m.reply_to && cardOf.get(m.reply_to);
    if (host) parent.set(m.id, host);
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
  const byAt = (a: Item, b: Item) => (a.at || 0) - (b.at || 0);
  const byId = new Map(items.map((i) => [i.id, i]));
  const main: Item[] = [];
  for (const item of items.sort(byAt)) {
    const top = parent.has(item.id) ? byId.get(root(item.id)) : undefined;
    if (top && top !== item) top.replies.push(item);
    else main.push(item);
  }
  return main;
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

/** What a turn's tool runs did, as its one line: "Read 4 files, ran 2 commands", "Ran 14 steps", "5 steps so far". */
export function stepsLine(steps: ToolBody[], running: boolean): string {
  const n = steps.length;
  if (running) return `${n} ${n === 1 ? 'step' : 'steps'} so far`;
  const counts = new Map<string, number>();
  let other = 0;
  for (const s of steps) {
    const kind = COUNTED[s.tool];
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
