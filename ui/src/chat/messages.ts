import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Agent, Failure, Message, Role, Task, ToolBody, Turn } from '../api';
import { fold, type Work } from './fold';
import { place } from './order';
import type { Approval, ChatState } from './state';
import { type Team, team } from './team';

/** A request that failed: what was sent (none for Stop or Pause), for Retry, and why. */
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

/** One of Gathr's notes, as a line: "Vale completed: Review PR #138". */
export interface EventLine {
  kind: 'task' | 'answer' | 'question' | 'note';
  /** Who did what, ahead of the text: "Vale completed:". */
  who?: string;
  text: string;
  /** What a click on the line adds: the reason, the question. */
  more?: string;
  task?: number;
}

/** Gathr's own part of a message, read by the thread's message components. */
export type Custom = {
  agent?: string;
  role?: Role;
  /** When the reply began, shown in a team chat. */
  time?: string;
  /** The speaker changed from the block before, so this one names it. */
  head?: boolean;
  /** The task a member's turn works on, after its name: "on Review PR #138". */
  on?: string;
  /** What the turn did before its message, as one row. */
  work?: Work;
  notice?: TurnNotice;
  approvals?: Approval[];
  /** A message that never reached the service. */
  unsent?: SendFailure;
  /** Messages between agents, one for each text of the block: "Juno to Orin". */
  exchange?: { id: string; from?: string; to?: string; time: string }[];
  events?: EventLine[];
};

/** The chat in order, and where a saved message shows in it, for a search hit. */
export interface Timeline {
  items: ThreadMessageLike[];
  find: (message: string) => Spot | undefined;
}

/** A thread message, and a hit's own text in it. */
export interface Spot {
  id: string;
  text?: string;
  /** Which of the block's texts equal to `text` it is: a reply can say the same thing twice. */
  nth?: number;
}

type Cache = Map<string, { sig: string; value: ThreadMessageLike }>;
export interface Group {
  key: string;
  turn: string | null;
  msgs: Message[];
  kind: 'person' | 'gathr' | 'reply' | 'to';
  /** Where its first message is among the chat's saved ones; one with none yet sits after its turn's latest. */
  at: number;
  /** It opens its turn: the message that started it, or its first reply. */
  bound: boolean;
  /** The turn's last reply: the one its streaming text, notice and approvals go on. */
  last?: boolean;
}

const noticed = new Set(['stopped', 'waiting_sign_in', 'failed']);
const engineWait = (t: Turn) => t.held === 'claude' || t.held === 'codex';
const hasNotice = (t: Turn) => noticed.has(t.status) || engineWait(t);

/**
 * Gathr's chat as assistant-ui messages, one timeline (0037). A turn is one block where it began: what it
 * did folds into a row, its last text is the message, and a later reply of the turn lands in that block,
 * so turns streaming at once never interleave. Only a person's message sent into a running turn splits it,
 * one block before and one after; the last holds the streaming text, the notice and the approvals.
 * Messages between agents are their own block where they were sent, shared while they follow each other;
 * Gathr's copy of one in the receiver's turn isn't shown. Of what Gathr told an agent, only what belongs
 * to no turn's row is drawn, as one-line events.
 *
 * A block keeps one id from its first streamed text to its saved messages, so nothing remounts when a
 * stream ends, and a block that didn't change keeps the same object, so assistant-ui doesn't convert it again.
 */
export function toThread(state: ChatState, agents: Agent[], unsent: SendFailure[], cache: Cache): Timeline {
  const crew = team(state, new Map(agents.map((a) => [a.id, a.name])));
  const groups = place(grouped(state), state);
  const gave = givenBy(state, crew);
  const items: ThreadMessageLike[] = [];
  /** Each group's block; one not drawn has none. */
  const shown = new Map<Group, { id: string; msgs: Message[] }>();
  const add = (value: ThreadMessageLike, from: Group[]) => {
    const msgs = from.flatMap((g) => g.msgs);
    for (const g of from) shown.set(g, { id: value.id as string, msgs });
    items.push(value);
  };
  let speaker: string | undefined;
  let talk: Group[] = [];
  const endTalk = () => {
    if (talk.length) add(exchange(cache, talk, state, crew), talk);
    talk = [];
  };
  for (const g of groups) {
    if (g.kind === 'to') {
      talk.push(g);
      speaker = undefined;
      continue;
    }
    if (g.kind === 'gathr') {
      const lines = heard(g, state, crew);
      if (!lines.length) continue;
      endTalk();
      add(eventsOf(cache, g.key, lines), [g]);
      speaker = undefined;
      continue;
    }
    endTalk();
    if (g.kind === 'person') {
      add(toUser(g.msgs[0]), [g]);
      speaker = undefined;
      continue;
    }
    const turn = g.turn ? state.turns[g.turn] : undefined;
    const on = turn?.task == null ? undefined : state.tasks.find((t) => t.number === turn.task)?.what;
    const who = `${turn?.agent}|${on}`;
    add(block(cache, g, state, crew, gave, { on, head: who !== speaker }), [g]);
    speaker = who;
    const asked = g.bound && g.turn ? questions(state, g.turn, crew) : [];
    if (asked.length) {
      items.push(eventsOf(cache, `asked-${g.turn}`, asked));
      speaker = undefined;
    }
  }
  endTalk();
  // A send that opened a turn fails that turn too, and the turn's notice already says so.
  const told = new Set(Object.values(state.turns).map((t) => t.ref));
  for (const f of unsent) if (!f.failure.ref || !told.has(f.failure.ref)) items.push(failed(f));

  const find = (message: string): Spot | undefined => {
    const hit = state.messages.find((x) => x.id === message);
    // Gathr's copy of an agent's message shows as the message it carries.
    const m = (hit?.copy_of && state.messages.find((x) => x.id === hit.copy_of)) || hit;
    if (!m) return undefined;
    const own = groups.find((g) => g.msgs.includes(m));
    const at = own && shown.get(own);
    if (at && m.kind !== 'tool' && own.kind !== 'gathr') {
      const before = at.msgs.slice(0, at.msgs.indexOf(m));
      const nth = before.filter((x) => x.kind !== 'tool' && x.body === m.body).length;
      return { id: at.id, text: m.body as string, nth };
    }
    if (at) return { id: at.id };
    // What Gathr told an agent and isn't drawn (the task it gave): its turn's block stands for it.
    const stand = m.turn && groups.find((g) => g.turn === m.turn && shown.has(g));
    return stand ? { id: shown.get(stand)?.id as string } : undefined;
  };
  return { items, find };
}

function grouped(state: ChatState): Group[] {
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
  state.messages.forEach((m, at) => {
    const bound = !!m.turn && !latest.has(m.turn);
    if (m.turn) latest.set(m.turn, at);
    const one = { turn: m.turn, msgs: [m], at };
    if (m.role === 'user') {
      groups.push({ ...one, key: m.id, kind: 'person', bound });
      if (m.turn) open.delete(m.turn);
    } else if (m.role === 'gathr') {
      if (!m.copy_of) groups.push({ ...one, key: m.id, kind: 'gathr', bound });
    } else if (m.to) {
      // An answer follows the message it answers: a turn that has only worked since that message
      // shows after what it sent, so the talk reads as one block.
      const sender = open.get(m.turn ?? '');
      const answering = groups.at(-1) === sender && groups.at(-2)?.kind === 'to';
      if (answering && sender?.msgs.every((x) => x.kind === 'tool')) sender.at = at + 0.5;
      groups.push({ ...one, key: m.id, kind: 'to', bound: false });
    } else if (m.turn && open.has(m.turn)) open.get(m.turn)?.msgs.push(m);
    else if (m.turn) {
      const g = { ...reply(m.turn, at), msgs: [m] };
      groups.push(g);
      open.set(m.turn, g);
    } else groups.push({ ...one, key: `a-${m.id}`, kind: 'reply', bound: false });
  });
  // A turn with streaming text, a notice or an approval but no reply since its latest message still
  // has one, right after that message.
  const waiting = new Set(state.approvals.map((a) => a.turn));
  for (const t of Object.values(state.turns)) {
    const last = open.get(t.id) ?? groups.findLast((g) => g.turn === t.id && g.kind !== 'to');
    if (last?.kind === 'reply') {
      last.last = true;
      continue;
    }
    if (!(state.partial[t.id] || hasNotice(t) || waiting.has(t.id))) continue;
    const after = latest.get(t.id);
    groups.push({ ...reply(t.id, after === undefined ? Number.POSITIVE_INFINITY : after + 0.5), last: true });
  }
  return groups;
}

/** The task each give_task call made: a turn's calls that worked and the tasks it gave, in order. */
function givenBy(state: ChatState, crew: Team) {
  const calls = new Map<string, Message[]>();
  for (const m of state.messages) {
    const body = m.kind === 'tool' ? (m.body as ToolBody) : undefined;
    if (!m.turn || body?.tool !== 'mcp__gathr__give_task' || body.status !== 'ok') continue;
    calls.set(m.turn, [...(calls.get(m.turn) ?? []), m]);
  }
  const gave = new Map<string, { task: Task; owner?: string }>();
  for (const [turn, made] of calls) {
    const tasks = state.tasks
      .filter((t) => t.given_in === turn && t.kind !== 'person')
      .sort((a, b) => a.number - b.number);
    // Only when they pair off: a call for each task, a task for each call.
    if (tasks.length !== made.length) continue;
    for (const [i, m] of made.entries()) gave.set(m.id, { task: tasks[i], owner: crew.name(tasks[i].owner) });
  }
  return gave;
}

function block(
  cache: Cache,
  group: Group,
  state: ChatState,
  crew: Team,
  gave: ReturnType<typeof givenBy>,
  { on, head }: { on?: string; head: boolean },
) {
  const id = group.turn;
  const turn = id ? state.turns[id] : undefined;
  const partial = group.last && id ? (state.partial[id] ?? '') : '';
  const approvals = group.last ? state.approvals.filter((a) => a.turn === id) : [];
  const agent = crew.name(turn?.agent);
  const role = crew.role(turn?.agent);
  const parts = group.msgs.map((m) => {
    if (m.kind !== 'tool') return m.id;
    const { status, duration_ms } = m.body as ToolBody;
    const made = gave.get(m.id);
    return `${m.id}:${status}:${duration_ms}:${made?.task.number}:${made?.task.what}:${made?.owner}`;
  });
  const ending = group.last ? `${turn?.status}|${turn?.ref}|${turn?.held}|${turn?.why}` : '';
  const who = `${agent}|${role}|${on}|${head}`;
  const asked = approvals.map((a) => `${a.request}:${a.answer?.allow}:${a.answer?.failure?.ref}`);
  const sig = `${parts}|${partial.length}|${ending}|${asked}|${who}`;
  const hit = cache.get(group.key);
  if (hit?.sig === sig) return hit.value;
  const time = role ? (group.msgs[0]?.time ?? turn?.created) : undefined;
  const custom: Custom = { agent, role, time, on, head, approvals };
  const value = toReply(group, partial, group.last ? turn : undefined, custom, gave);
  cache.set(group.key, { sig, value });
  return value;
}

function toReply(
  group: Group,
  partial: string,
  turn: Turn | undefined,
  custom: Custom,
  gave: ReturnType<typeof givenBy>,
): ThreadMessageLike {
  const status = turn?.status;
  const began = group.bound ? turn?.created : undefined;
  const { work, text } = fold(group.msgs, partial, status === 'running', gave, began);
  if (work) custom.work = work;
  if (turn && hasNotice(turn)) custom.notice = { turn, replied: group.msgs.length > 0 || !!partial };
  return {
    id: group.key,
    role: 'assistant',
    content: text ? [{ type: 'text', text }] : [],
    status:
      status === 'running' || (status === 'queued' && !(turn && engineWait(turn)))
        ? { type: 'running' }
        : status === 'failed'
          ? { type: 'incomplete', reason: 'error' }
          : status === 'stopped'
            ? { type: 'incomplete', reason: 'cancelled' }
            : { type: 'complete', reason: 'stop' },
    metadata: { custom },
  };
}

/** Messages between agents that follow each other, as one block: a text and its header for each. */
function exchange(cache: Cache, groups: Group[], state: ChatState, crew: Team): ThreadMessageLike {
  const key = groups[0].key;
  const said = groups.map(({ msgs: [m] }) => ({
    id: m.id,
    from: crew.name(state.turns[m.turn ?? '']?.agent),
    to: crew.name(m.to),
    time: m.time,
  }));
  const sig = JSON.stringify(said);
  const hit = cache.get(key);
  if (hit?.sig === sig) return hit.value;
  const custom: Custom = { exchange: said };
  const value: ThreadMessageLike = {
    id: key,
    role: 'assistant',
    content: groups.map(({ msgs: [m] }) => ({ type: 'text', text: m.body as string })),
    metadata: { custom },
  };
  cache.set(key, { sig, value });
  return value;
}

const SETTLED: Partial<Record<Task['state'], string>> = {
  done: 'completed',
  failed: 'failed',
  stopped: 'stopped',
  cancelled: 'cancelled',
  blocked: 'is blocked on',
};

/**
 * What Gathr told an agent, as the lines worth drawing: the person's answers it heard, the tasks a
 * hear-back reports, else its own words. None for what opens a member's turn on a task: the member's
 * block is headed by the task.
 */
function heard(group: Group, state: ChatState, crew: Team): EventLine[] {
  const m = group.msgs[0];
  const text = m.body as string;
  const turn = state.turns[m.turn ?? ''];
  if (!turn) return [{ kind: 'note', text }];
  const answers = state.tasks
    .filter((t) => t.kind === 'person' && t.state === 'done' && t.heard_in === turn.id)
    .map(
      (t): EventLine => ({
        kind: 'answer',
        who: `You answered ${crew.name(t.giver) ?? 'an agent'}:`,
        text: t.reason ?? 'Done',
        more: `Question #${t.number}: ${t.what}`,
        task: t.number,
      }),
    );
  if (answers.length) return answers;
  // A hear-back names each task it reports, of the ones the turn it answers gave; its own task's
  // number is in the message that gave it.
  const reported = (t: Task) =>
    t.kind !== 'person' &&
    t.given_in === turn.answers &&
    t.number !== turn.task &&
    text.includes(`#${t.number} `);
  const back = state.tasks.filter(reported).map(
    (t): EventLine => ({
      kind: 'task',
      who: `${crew.name(t.owner) ?? 'An agent'} ${SETTLED[t.state] ?? 'is working on'}:`,
      text: t.what,
      more: [`Task #${t.number}`, t.reason].filter(Boolean).join('. '),
      task: t.number,
    }),
  );
  if (back.length) return back;
  // What opens a turn on a task is the task itself, or a wake-up to carry on with it.
  return turn.task != null && group.bound ? [] : [{ kind: 'note', text }];
}

/** The questions to the person a turn asked. */
function questions(state: ChatState, turn: string, crew: Team): EventLine[] {
  return state.tasks
    .filter((t) => t.kind === 'person' && t.given_in === turn)
    .map(
      (t): EventLine => ({
        kind: 'question',
        who: `${crew.name(t.giver) ?? 'An agent'} asked you:`,
        text: t.what,
        more: [`Question #${t.number}`, t.why].filter(Boolean).join('. '),
        task: t.number,
      }),
    );
}

function eventsOf(cache: Cache, key: string, events: EventLine[]): ThreadMessageLike {
  const sig = JSON.stringify(events);
  const hit = cache.get(key);
  if (hit?.sig === sig) return hit.value;
  const text = events.map((e) => [e.who, e.text].filter(Boolean).join(' ')).join('\n');
  const custom: Custom = { events };
  const value: ThreadMessageLike = {
    id: key,
    role: 'system',
    content: [{ type: 'text', text }],
    metadata: { custom },
  };
  cache.set(key, { sig, value });
  return value;
}

function toUser(m: Message): ThreadMessageLike {
  return {
    id: m.id,
    role: 'user',
    content: [{ type: 'text', text: m.body as string }],
    metadata: { custom: {} },
  };
}

function failed(f: SendFailure): ThreadMessageLike {
  const custom: Custom = { unsent: f };
  return { id: `f-${f.id}`, role: 'user', content: [{ type: 'text', text: f.text }], metadata: { custom } };
}
