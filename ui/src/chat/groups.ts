import type { Message, ToolBody, Turn } from '../api';
import type { Given } from './fold';
import type { ChatState } from './state';
import type { Team } from './team';

/** A run of saved messages drawn as one block, or a question or answer that has no message of its own. */
export interface Group {
  key: string;
  turn: string | null;
  msgs: Message[];
  kind: 'person' | 'gathr' | 'reply' | 'to' | 'question' | 'said';
  /** Where its first message is among the chat's saved ones; one with none yet sits after its turn's latest. */
  at: number;
  /** It opens its turn: the message that started it, or its first reply. */
  bound: boolean;
  /** The turn's last reply: the one its streaming text, notice and approvals go on. */
  last?: boolean;
  /** A message to an agent: the block its turn was writing when it was sent. */
  root?: Group;
  /** The turn it began in, when an engine's own pick-up of that turn's work carries it on: `turn` is the pick-up. */
  first?: string;
  /** A question that is a block of its own, or the person's answer or Discuss to it, which has no message. */
  task?: number;
  text?: string;
  time?: string;
}

const ended = new Set(['stopped', 'waiting_sign_in', 'failed']);
export const hasEnding = (t: Turn) => ended.has(t.status);

/**
 * The turn whose block a turn's messages go in: its own, or for an engine's own pick-up (its engine went on with the
 * work by itself, when a command it left running ended) the turn of the same agent it carries on. One piece of work
 * reads as one message (the owner's call of 2026-11-19).
 */
function blockOf(turns: ChatState['turns']) {
  const of = (id: string, hops = 0): string => {
    const t = turns[id];
    const before = t?.opened === 'engine' && t.answers ? turns[t.answers] : undefined;
    return before && before.agent === t?.agent && hops < 50 ? of(before.id, hops + 1) : id;
  };
  return of;
}

/**
 * The chat's saved messages as blocks, in saved order (0037). A turn is one block where it began: its words and the
 * runs of steps between them, a plan version it wrote among them (0078), and a later reply of the turn lands in that
 * block, so turns streaming at once never interleave. Only a person's message sent into a running turn, or Gathr's copy
 * of one, splits it, one block before and one after.
 * Each message to an agent is a block of its own. A block keeps one id from its first streamed text to its saved
 * messages, so nothing remounts when a stream ends.
 */
export function grouped(state: ChatState): Group[] {
  // The nth reply of a turn is `a-<turn>`, then `a-<turn>-<n>`: history only grows at its end, so n is stable.
  const counts = new Map<string, number>();
  const reply = (turn: string, at: number): Group => {
    const n = counts.get(turn) ?? 0;
    counts.set(turn, n + 1);
    return { key: n ? `a-${turn}-${n}` : `a-${turn}`, turn, msgs: [], kind: 'reply', at, bound: n === 0 };
  };
  const groups: Group[] = [];
  // Each turn's reply that its next message joins, until a person's message sent into the turn splits it.
  const open = new Map<string, Group>();
  // Each turn's latest saved message, of any kind.
  const latest = new Map<string, number>();
  const persons = new Set(state.messages.filter((m) => m.role === 'user').map((m) => m.id));
  const blockTurn = blockOf(state.turns);
  /** A pick-up's message lands in the block of the turn it carries on, which is then the pick-up's to end. */
  const carryOn = (g: Group, turn: string) => {
    if (g.turn === turn) return;
    g.first ??= g.turn ?? undefined;
    g.turn = turn;
  };
  state.messages.forEach((m, at) => {
    const bound = !!m.turn && !latest.has(m.turn);
    if (m.turn) latest.set(m.turn, at);
    const one = { turn: m.turn, msgs: [m], at };
    const key = m.turn ? blockTurn(m.turn) : '';
    const going = open.get(key);
    if (m.role === 'user') {
      groups.push({ ...one, key: m.id, kind: 'person', bound });
      open.delete(key);
    } else if (m.role === 'gathr') {
      if (!m.copy_of) groups.push({ ...one, key: m.id, kind: 'gathr', bound });
      // Gathr's copy of the person's message splits the turn it entered as theirs does (0028); it draws nothing.
      else if (m.turn && persons.has(m.copy_of)) open.delete(key);
    } else if (m.to) {
      groups.push({ ...one, key: m.id, kind: 'to', bound: false, root: going });
    } else if (m.turn && going) {
      going.msgs.push(m);
      carryOn(going, m.turn);
    } else if (m.turn) {
      const g = { ...reply(key, at), msgs: [m] };
      carryOn(g, m.turn);
      groups.push(g);
      open.set(key, g);
    } else groups.push({ ...one, key: `a-${m.id}`, kind: 'reply', bound: false });
  });
  // A turn running, or with streaming text, a notice or an approval, but no reply since its latest message still
  // has one, right after that message: a running one shows as its agent at work from its start.
  const waiting = new Set(state.approvals.map((a) => a.turn));
  const made = (id: string | null | undefined) => state.turns[id ?? '']?.created ?? '';
  for (const t of Object.values(state.turns)) {
    const key = blockTurn(t.id);
    const last =
      open.get(key) ?? groups.findLast((g) => g.kind !== 'to' && !!g.turn && blockTurn(g.turn) === key);
    if (last?.kind === 'reply') {
      last.last = true;
      // A pick-up with nothing saved yet takes the block over: it is the turn at work there now.
      if (key !== t.id && made(t.id) >= made(last.turn)) carryOn(last, t.id);
      continue;
    }
    if (!(t.status === 'running' || state.partial[t.id] || hasEnding(t) || waiting.has(t.id))) continue;
    const after = latest.get(t.id);
    const g = { ...reply(key, after === undefined ? Number.POSITIVE_INFINITY : after + 0.5), last: true };
    carryOn(g, t.id);
    groups.push(g);
  }
  return groups;
}

/** Each task's last block of a turn on it: how the task ended is said there, once (0051). */
export function lastBlocks(groups: Group[], state: ChatState): Map<number, Group> {
  const last = new Map<number, Group>();
  for (const g of groups) {
    const task = g.kind === 'reply' ? state.turns[g.turn ?? '']?.task : undefined;
    if (task != null) last.set(task, g);
  }
  return last;
}

/** The task each give_task call made: a turn's calls that worked and the tasks it gave, in order. */
export function givenBy(state: ChatState, crew: Team) {
  const calls = new Map<string, Message[]>();
  for (const m of state.messages) {
    const body = m.kind === 'tool' ? (m.body as ToolBody) : undefined;
    if (!m.turn || body?.tool !== 'mcp__gathr__give_task' || body.status !== 'ok') continue;
    calls.set(m.turn, [...(calls.get(m.turn) ?? []), m]);
  }
  const gave = new Map<string, Given>();
  for (const [turn, made] of calls) {
    const tasks = state.tasks
      // A plan's check and its parts' tasks are Gathr's gives, with no call of the turn's.
      .filter((t) => t.given_in === turn && t.kind !== 'person' && !t.checks && t.plan == null)
      .sort((a, b) => a.number - b.number);
    // Only when they pair off: a call for each task, a task for each call.
    if (tasks.length !== made.length) continue;
    for (const [i, m] of made.entries()) gave.set(m.id, { task: tasks[i], owner: crew.name(tasks[i].owner) });
  }
  return gave;
}
