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
 * they started; a reply, the service's or the person's, under the main message it answers. Gathr's
 * own lines aren't shown, and a turn that said nothing and ended shows nothing.
 */
export function shape(s: RoomState): Item[] {
  const byTurn = new Map<string, Message[]>();
  // The person's message that opened a turn: the service makes the turn just before it, and its card goes after it.
  const opener = new Map<string, number>();
  for (const m of s.messages) {
    if (m.role === 'user' && m.turn) opener.set(m.turn, time(m.time));
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
    const at = Math.max(time(turn.created ?? mine[0]?.time), opener.get(turn.id) ?? 0);
    cards.set(turn.id, { kind: 'card', id: turn.id, turn, text, said, steps, at, replies: [] });
  }
  const cardOf = new Map<string, string>();
  for (const c of cards.values()) for (const m of byTurn.get(c.id) ?? []) cardOf.set(m.id, c.id);

  // Where each item sits: under the main message its `reply_to` names (an agent's card or the person's message),
  // or in the main chat.
  const parent = new Map<string, string>();
  const under = (id: string, to?: string | null) => {
    const host = to && (cardOf.get(to) ?? to);
    if (host && host !== id) parent.set(id, host);
  };
  for (const c of cards.values())
    under(c.id, byTurn.get(c.id)?.find((m) => m.reply_to)?.reply_to ?? s.under[c.id]);
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
