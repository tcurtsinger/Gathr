import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Agent, Failure, Message, Role, Task, ToolBody, Turn } from '../api';
import type { Approval, ChatState } from './state';
import { describe, finished, type Team, team } from './team';

/** A request that failed: what was sent (none for Stop or an approval), for Retry, and why. */
export interface SendFailure {
  id: string;
  text: string;
  /** The agent it was for, so a resend goes to the same one. */
  agent?: string;
  failure: Failure;
}

/** What a turn's reply shows under its text: a notice for a turn that stopped, waits or failed. */
export interface TurnNotice {
  turn: Turn;
  /** Some of the reply arrived, so a failure reads "Reply failed", not "Send failed". */
  replied: boolean;
}

/** A team thread's disclosure: who is working, waiting and done, and whether the round is over. */
export interface ThreadHead {
  root: string;
  text: string;
  done: boolean;
  /** No member has started yet, so nothing sits under it. */
  alone: boolean;
}

/** Gathr's own part of a message, read by the thread's message components. */
export type Custom = {
  agent?: string;
  role?: Role;
  /** When the reply began, shown in a team chat. */
  time?: string;
  notice?: TurnNotice;
  approvals?: Approval[];
  /** A message that never reached the service. */
  unsent?: SendFailure;
  /** The thread it sits in, by its root turn; `first` and `end` start and close it. */
  thread?: string;
  first?: boolean;
  end?: boolean;
  head?: ThreadHead;
  /** What Gathr told an agent outside a task: a wake-up. */
  gathr?: string;
  /** The task a member's turn works on: "Mira to Rowan · Cursor commit". */
  assignment?: { from?: string; to?: string; what: string };
  /** The lead's last word on a round: its thread. */
  back?: { root: string; done: boolean };
};

type Part = Exclude<ThreadMessageLike['content'], string>[number];
type Cache = Map<string, { sig: string; value: ThreadMessageLike }>;
interface Group {
  key: string;
  turn: string | null;
  msgs: Message[];
  reply: boolean;
  /** What Gathr told the turn's agent. */
  gathr?: boolean;
  /** The turn's last reply: the one its streaming text, notice and approvals go on. */
  last?: boolean;
}

const noticed = new Set(['stopped', 'waiting_sign_in', 'failed']);
const settled = new Set(['done', 'blocked', 'failed', 'stopped', 'cancelled']);

/**
 * Gathr's chat as assistant-ui messages: each person's message, then the reply to it holding its
 * text and tool calls in order. A message sent into a running turn splits that turn's reply in two,
 * one before it and one after; the last holds the streaming text, the notice and the approvals.
 * A reply keeps one id from its first streamed text to its saved messages, so nothing remounts when
 * a stream ends, and a reply whose parts didn't change keeps the same object, so assistant-ui doesn't
 * convert it again.
 *
 * In a team chat, the work on each task sits in one thread right after the lead's message that gave
 * it, under a disclosure item (`t-<turn>`), wherever the work's turns fall in time.
 */
export function toThread(
  state: ChatState,
  agents: Agent[],
  unsent: SendFailure[],
  cache: Cache,
): ThreadMessageLike[] {
  const crew = team(state, new Map(agents.map((a) => [a.id, a.name])));
  const groups = grouped(state);
  const main: Group[] = [];
  const threaded = new Map<string, Group[]>();
  for (const g of groups) {
    const root = g.turn ? crew.rootOf(g.turn) : undefined;
    if (root) threaded.set(root, [...(threaded.get(root) ?? []), g]);
    else main.push(g);
  }
  // Replies streaming at once save in the order they finish: a thread keeps its turns' order, as a reload does.
  const order = new Map(Object.keys(state.turns).map((id, i) => [id, i]));
  for (const list of threaded.values())
    list.sort((a, b) => (order.get(a.turn ?? '') ?? 0) - (order.get(b.turn ?? '') ?? 0));
  const roots = new Set([...crew.threads.keys(), ...threaded.keys()]);
  // The lead's word on a round: a main-line turn Gathr started that answers the turn with a thread.
  const roundOf = (turn: string | null) => {
    const answers = turn ? state.turns[turn]?.answers : undefined;
    const root = answers ? (crew.rootOf(answers) ?? answers) : undefined;
    return root && roots.has(root) ? root : undefined;
  };
  // A round's first word, and its newest: a reply that came late brings the lead back with another.
  const firsts = new Map<string, Turn>();
  const finals = new Map<string, Turn>();
  for (const turn of Object.values(state.turns)) {
    const root = crew.rootOf(turn.id) ? undefined : roundOf(turn.id);
    if (!root) continue;
    if (!firsts.has(root)) firsts.set(root, turn);
    finals.set(root, turn);
  }
  const newest = (turn: string | null) => {
    const root = roundOf(turn);
    return root && finals.get(root)?.id === turn ? root : undefined;
  };
  // What Gathr told the lead about its round reads as the link on its word, not as words of their own.
  const lines = main.filter((g) => !(g.gathr && roundOf(g.turn)));
  main.length = 0;
  main.push(...lines);
  const places = new Map<number, string[]>();
  for (const root of roots) {
    const at = main.findLastIndex((g) => g.turn === root);
    const i = at < 0 ? main.length - 1 : at;
    places.set(i, [...(places.get(i) ?? []), root]);
  }

  const out: ThreadMessageLike[] = [];
  const place = (i: number) => {
    for (const root of places.get(i) ?? []) {
      const members = threaded.get(root) ?? [];
      out.push(head(cache, root, members, state, crew, firsts.get(root), finals.get(root)));
      members.forEach((g, n) => {
        const ends = { first: n === 0, end: n === members.length - 1 };
        out.push(cached(cache, g, state, crew, { thread: root, ...ends }));
      });
    }
  };
  place(-1);
  main.forEach((g, i) => {
    out.push(cached(cache, g, state, crew, g.last ? lastWord(g, newest(g.turn), state, crew) : {}));
    place(i);
  });
  // A send that opened a turn fails that turn too, and the turn's notice already says so.
  const told = new Set(Object.values(state.turns).map((t) => t.ref));
  for (const f of unsent) if (!f.failure.ref || !told.has(f.failure.ref)) out.push(failed(f));
  return out;
}

function grouped(state: ChatState): Group[] {
  // The nth reply of a turn is `a-<turn>`, then `a-<turn>-<n>`: history only grows at its end, so n is stable.
  const counts = new Map<string, number>();
  const reply = (turn: string): Group => {
    const n = counts.get(turn) ?? 0;
    counts.set(turn, n + 1);
    return { key: n ? `a-${turn}-${n}` : `a-${turn}`, turn, msgs: [], reply: true };
  };
  const groups: Group[] = [];
  for (const m of state.messages) {
    const prev = groups.at(-1);
    if (m.role === 'user') groups.push({ key: m.id, turn: m.turn, msgs: [m], reply: false });
    else if (m.role === 'gathr')
      groups.push({ key: m.id, turn: m.turn, msgs: [m], reply: false, gathr: true });
    else if (m.turn && prev?.reply && prev.turn === m.turn) prev.msgs.push(m);
    else if (m.turn) groups.push({ ...reply(m.turn), msgs: [m] });
    else groups.push({ key: `a-${m.id}`, turn: null, msgs: [m], reply: true });
  }
  // A turn with streaming text, a notice or an approval but no reply since its latest message still
  // has one, right after that message.
  const waiting = new Set(state.approvals.map((a) => a.turn));
  for (const t of Object.values(state.turns)) {
    const at = groups.findLastIndex((g) => g.turn === t.id);
    if (at >= 0 && groups[at].reply) {
      groups[at].last = true;
      continue;
    }
    if (!(state.partial[t.id] || noticed.has(t.status) || waiting.has(t.id))) continue;
    groups.splice(at < 0 ? groups.length : at + 1, 0, { ...reply(t.id), last: true });
  }
  return groups;
}

/**
 * On the lead's last word of a round: the link back to its thread. No reviewer's agreement: a review
 * done says what it reviewed, not whether it approved, and the service records no verdict yet.
 */
function lastWord(g: Group, back: string | undefined, state: ChatState, crew: Team): Custom {
  if (!back || !g.reply) return {};
  return { back: { root: back, done: roundOver(state.turns[g.turn ?? ''], crew.threads.get(back) ?? []) } };
}

const roundOver = (final: Turn | undefined, tasks: Task[]) =>
  finished(final) && tasks.every((t) => settled.has(t.state));

function head(
  cache: Cache,
  root: string,
  members: Group[],
  state: ChatState,
  crew: Team,
  first: Turn | undefined,
  final: Turn | undefined,
): ThreadMessageLike {
  const tasks = crew.threads.get(root) ?? [];
  const done = roundOver(final, tasks);
  // A reply that began after the lead's first word on the round began came late.
  const began = (turn: string | null | undefined) =>
    Date.parse(state.messages.find((m) => m.turn === turn)?.time ?? '');
  const after = began(first?.id);
  const late = members
    .filter((g) => g.reply && began(g.turn) > after)
    .map((g) => ({ agent: state.turns[g.turn as string]?.agent ?? '', time: g.msgs[0]?.time ?? '' }))
    .filter((l, i, all) => l.agent && all.findLastIndex((x) => x.agent === l.agent) === i);
  const text = describe(tasks, late, crew.name);
  const key = `t-${root}`;
  const alone = members.length === 0;
  const sig = `${text}|${done}|${alone}`;
  const hit = cache.get(key);
  if (hit?.sig === sig) return hit.value;
  const custom: Custom = { head: { root, text, done, alone } };
  const value: ThreadMessageLike = {
    id: key,
    role: 'system',
    content: [{ type: 'text', text: 'Team thread' }],
    metadata: { custom },
  };
  cache.set(key, { sig, value });
  return value;
}

function cached(cache: Cache, group: Group, state: ChatState, crew: Team, extra: Custom) {
  const id = group.turn;
  const turn = id ? state.turns[id] : undefined;
  const partial = group.last && id ? (state.partial[id] ?? '') : '';
  const approvals = group.last ? state.approvals.filter((a) => a.turn === id) : [];
  const agent = crew.name(turn?.agent);
  const role = crew.role(turn?.agent);
  const parts = group.msgs.map((m) => `${m.id}:${m.kind === 'tool' ? (m.body as ToolBody).status : ''}`);
  const ending = group.last ? `${turn?.status}|${turn?.ref}` : '';
  const task = group.gathr ? taskLine(turn, state, crew) : undefined;
  const who = `${agent}|${role}|${JSON.stringify(task)}|${JSON.stringify(extra)}`;
  const sig = `${parts}|${partial.length}|${ending}|${approvals.map((a) => a.request)}|${who}`;
  const hit = cache.get(group.key);
  if (hit?.sig === sig) return hit.value;
  const value = group.gathr
    ? toGathr(group.msgs[0], task, extra)
    : group.reply
      ? toReply(group, partial, group.last ? turn : undefined, approvals, {
          ...extra,
          agent,
          role,
          time: role ? (group.msgs[0]?.time ?? turn?.created) : undefined,
        })
      : toUser(group.msgs[0], extra);
  cache.set(group.key, { sig, value });
  return value;
}

/** The task a member's turn works on, from Gathr's message that started it. */
function taskLine(turn: Turn | undefined, state: ChatState, crew: Team): Custom['assignment'] {
  const task = turn?.task == null ? undefined : state.tasks.find((t) => t.number === turn.task);
  if (!task) return undefined;
  return { from: crew.name(task.giver), to: crew.name(task.owner), what: task.what };
}

function toUser(m: Message, extra: Custom): ThreadMessageLike {
  const custom: Custom = { ...extra };
  return {
    id: m.id,
    role: 'user',
    content: [{ type: 'text', text: m.body as string }],
    metadata: { custom },
  };
}

function toGathr(m: Message, assignment: Custom['assignment'], extra: Custom): ThreadMessageLike {
  const text = m.body as string;
  const custom: Custom = assignment ? { ...extra, assignment } : { ...extra, gathr: text };
  return { id: m.id, role: 'system', content: [{ type: 'text', text }], metadata: { custom } };
}

function failed(f: SendFailure): ThreadMessageLike {
  const custom: Custom = { unsent: f };
  return { id: `f-${f.id}`, role: 'user', content: [{ type: 'text', text: f.text }], metadata: { custom } };
}

function toReply(
  group: Group,
  partial: string,
  turn: Turn | undefined,
  approvals: Approval[],
  extra: Custom,
): ThreadMessageLike {
  const content: Part[] = group.msgs.map((m) => {
    if (m.kind !== 'tool') return { type: 'text', text: m.body as string };
    const body = m.body as ToolBody;
    const startedAt = Date.parse(m.time);
    const completedAt = body.duration_ms === undefined ? undefined : startedAt + body.duration_ms;
    return {
      type: 'tool-call',
      toolCallId: m.id,
      toolName: body.tool,
      args: {},
      result: body.status === 'running' ? undefined : body.status,
      isError: body.status === 'error',
      timing: Number.isNaN(startedAt) ? undefined : { startedAt, completedAt },
    };
  });
  if (partial) content.push({ type: 'text', text: partial });
  const status = turn?.status;
  const custom: Custom = { ...extra, approvals };
  if (turn && noticed.has(turn.status)) custom.notice = { turn, replied: content.length > 0 };
  return {
    id: group.key,
    role: 'assistant',
    content,
    status:
      status === 'running' || status === 'queued'
        ? { type: 'running' }
        : status === 'failed'
          ? { type: 'incomplete', reason: 'error' }
          : status === 'stopped'
            ? { type: 'incomplete', reason: 'cancelled' }
            : { type: 'complete', reason: 'stop' },
    metadata: { custom },
  };
}
