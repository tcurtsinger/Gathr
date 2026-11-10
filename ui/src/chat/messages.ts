import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Agent, Failure, Message, Role, ToolBody, Turn } from '../api';
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
  /** What Gathr told an agent outside a task: a wake-up. */
  gathr?: string;
  /** The task a member's turn works on: "Mira to Rowan · Cursor commit". */
  assignment?: { from?: string; to?: string; what: string };
};

/** The chat's two columns: the Supervisor's conversation, and the team's work on hand-offs. */
export interface Columns {
  main: ThreadMessageLike[];
  team: ThreadMessageLike[];
}

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
const engineWait = (t: Turn) => t.held === 'claude' || t.held === 'codex';
const hasNotice = (t: Turn) => noticed.has(t.status) || engineWait(t);

/**
 * Gathr's chat as assistant-ui messages, in two columns. Each person's message, then the reply to it
 * holding its text and tool calls in order: a turn is one reply wherever its messages land among other
 * agents' and Gathr's, but a person's message sent into a running turn of the same column splits it in
 * two, one before it and one after; the last holds the streaming text, the notice and the approvals.
 * A reply keeps one id from its first streamed text to its saved messages, so nothing remounts when
 * a stream ends, and a reply whose parts didn't change keeps the same object, so assistant-ui doesn't
 * convert it again.
 *
 * Work on a hand-off (a turn on a task, or one hearing back inside it) goes to the team column, in the
 * order its turns began, so replies streaming at once never interleave (0015, 0028).
 */
export function toThread(state: ChatState, agents: Agent[], unsent: SendFailure[], cache: Cache): Columns {
  const crew = team(state, new Map(agents.map((a) => [a.id, a.name])));
  const inTeam = (turn: string | null) => !!turn && !!crew.rootOf(turn);
  const main: ThreadMessageLike[] = [];
  const work: Group[] = [];
  for (const g of grouped(state, inTeam)) {
    if (inTeam(g.turn)) work.push(g);
    // What Gathr told the lead when a round settled: the team column and Summary already show it.
    else if (!(g.gathr && state.turns[g.turn ?? '']?.answers)) main.push(cached(cache, g, state, crew));
  }
  // Replies streaming at once save in the order they finish: the team keeps its turns' order, as a reload does.
  const order = new Map(Object.keys(state.turns).map((id, i) => [id, i]));
  work.sort((a, b) => (order.get(a.turn ?? '') ?? 0) - (order.get(b.turn ?? '') ?? 0));
  // A send that opened a turn fails that turn too, and the turn's notice already says so.
  const told = new Set(Object.values(state.turns).map((t) => t.ref));
  for (const f of unsent) if (!f.failure.ref || !told.has(f.failure.ref)) main.push(failed(f));
  return { main, team: work.map((g) => cached(cache, g, state, crew)) };
}

function grouped(state: ChatState, inTeam: (turn: string | null) => boolean): Group[] {
  // The nth reply of a turn is `a-<turn>`, then `a-<turn>-<n>`: history only grows at its end, so n is stable.
  const counts = new Map<string, number>();
  const reply = (turn: string): Group => {
    const n = counts.get(turn) ?? 0;
    counts.set(turn, n + 1);
    return { key: n ? `a-${turn}-${n}` : `a-${turn}`, turn, msgs: [], reply: true };
  };
  const groups: Group[] = [];
  // Each turn's reply that its next message joins, until a person's message in its column splits it.
  const open = new Map<string, Group>();
  for (const m of state.messages) {
    if (m.role === 'user') {
      groups.push({ key: m.id, turn: m.turn, msgs: [m], reply: false });
      for (const turn of open.keys()) if (inTeam(turn) === inTeam(m.turn)) open.delete(turn);
    } else if (m.role === 'gathr')
      groups.push({ key: m.id, turn: m.turn, msgs: [m], reply: false, gathr: true });
    else if (m.turn && open.has(m.turn)) open.get(m.turn)?.msgs.push(m);
    else if (m.turn) {
      const g = { ...reply(m.turn), msgs: [m] };
      groups.push(g);
      open.set(m.turn, g);
    } else groups.push({ key: `a-${m.id}`, turn: null, msgs: [m], reply: true });
  }
  // A turn with streaming text, a notice or an approval but no reply since its latest message still
  // has one, right after that message.
  const waiting = new Set(state.approvals.map((a) => a.turn));
  for (const t of Object.values(state.turns)) {
    const at = groups.findLastIndex((g) => g.turn === t.id);
    const last = open.get(t.id) ?? groups[at];
    if (last?.reply) {
      last.last = true;
      continue;
    }
    if (!(state.partial[t.id] || hasNotice(t) || waiting.has(t.id))) continue;
    groups.splice(at < 0 ? groups.length : at + 1, 0, { ...reply(t.id), last: true });
  }
  return groups;
}

function cached(cache: Cache, group: Group, state: ChatState, crew: Team) {
  const id = group.turn;
  const turn = id ? state.turns[id] : undefined;
  const partial = group.last && id ? (state.partial[id] ?? '') : '';
  const approvals = group.last ? state.approvals.filter((a) => a.turn === id) : [];
  const agent = crew.name(turn?.agent);
  const role = crew.role(turn?.agent);
  const parts = group.msgs.map((m) => `${m.id}:${m.kind === 'tool' ? (m.body as ToolBody).status : ''}`);
  const ending = group.last ? `${turn?.status}|${turn?.ref}|${turn?.held}|${turn?.why}` : '';
  const task = group.gathr ? taskLine(turn, state, crew) : undefined;
  const who = `${agent}|${role}|${JSON.stringify(task)}`;
  const asked = approvals.map((a) => `${a.request}:${a.answer?.allow}:${a.answer?.failure?.ref}`);
  const sig = `${parts}|${partial.length}|${ending}|${asked}|${who}`;
  const hit = cache.get(group.key);
  if (hit?.sig === sig) return hit.value;
  const value = group.gathr
    ? toGathr(group.msgs[0], task)
    : group.reply
      ? toReply(group, partial, group.last ? turn : undefined, approvals, {
          agent,
          role,
          time: role ? (group.msgs[0]?.time ?? turn?.created) : undefined,
        })
      : toUser(group.msgs[0]);
  cache.set(group.key, { sig, value });
  return value;
}

/** The task a member's turn works on, from Gathr's message that started it. */
function taskLine(turn: Turn | undefined, state: ChatState, crew: Team): Custom['assignment'] {
  const task = turn?.task == null ? undefined : state.tasks.find((t) => t.number === turn.task);
  if (!task) return undefined;
  return { from: crew.name(task.giver), to: crew.name(task.owner), what: task.what };
}

function toUser(m: Message): ThreadMessageLike {
  return {
    id: m.id,
    role: 'user',
    content: [{ type: 'text', text: m.body as string }],
    metadata: { custom: {} },
  };
}

function toGathr(m: Message, assignment: Custom['assignment']): ThreadMessageLike {
  const text = m.body as string;
  const custom: Custom = assignment ? { assignment } : { gathr: text };
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
  if (turn && hasNotice(turn)) custom.notice = { turn, replied: content.length > 0 };
  return {
    id: group.key,
    role: 'assistant',
    content,
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
