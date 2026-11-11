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
  /** Whom an agent's message is for: "To Pia". */
  to?: string;
  /** The task a member's turn works on: "Mira to Rowan · Cursor commit". */
  assignment?: { number: number; from?: string; to?: string; what: string; why?: string };
  /** The person's answers its giver heard in this turn: "You answered #26 · B", and the question. */
  answered?: { line: string; asked: string }[];
  /** Questions to the person asked in a team turn: "Mira asked you #95 · Ship it?", and why (G160). */
  questions?: { line: string; why?: string }[];
};

/** The chat's two columns: the Supervisor's conversation, and the team's work on hand-offs. */
export interface Columns {
  main: ThreadMessageLike[];
  team: ThreadMessageLike[];
  /** Where a saved message shows, for a search hit: its column, its thread message, and its text in a reply. */
  find: (message: string) => Spot | undefined;
}

export interface Spot {
  column: 'main' | 'team';
  id: string;
  text?: string;
  /** Which of the reply's texts equal to `text` it is: a reply can say the same thing twice. */
  nth?: number;
}

type Part = Exclude<ThreadMessageLike['content'], string>[number];
type Cache = Map<string, { sig: string; value: ThreadMessageLike }>;
interface Group {
  key: string;
  turn: string | null;
  msgs: Message[];
  reply: boolean;
  /** The person's message: always in the main column, even one sent into a member's turn on a task. */
  person?: boolean;
  /** What Gathr told the turn's agent. */
  gathr?: boolean;
  /** The turn's last reply: the one its streaming text, notice and approvals go on. */
  last?: boolean;
  /** An agent's message to one member: their id. */
  to?: string;
}

const noticed = new Set(['stopped', 'waiting_sign_in', 'failed']);
const engineWait = (t: Turn) => t.held === 'claude' || t.held === 'codex';
const hasNotice = (t: Turn) => noticed.has(t.status) || engineWait(t);

/**
 * Gathr's chat as assistant-ui messages, in two columns. Each person's message, then the reply to it
 * holding its text and tool calls in order: a turn is one reply wherever its messages land among other
 * agents' and Gathr's, but a person's message sent into a running turn of the main column splits it in
 * two, one before it and one after; the last holds the streaming text, the notice and the approvals.
 * A reply keeps one id from its first streamed text to its saved messages, so nothing remounts when
 * a stream ends, and a reply whose parts didn't change keeps the same object, so assistant-ui doesn't
 * convert it again.
 *
 * Work on a hand-off (a turn on a task, or one hearing back inside it) goes to the team column, in the
 * order its turns began, so replies streaming at once never interleave (0015, 0028). So does an agent's
 * message to one member, at its turn's place, even from the lead's main-line turn; Gathr's copy of it in
 * the member's turn isn't shown. The person's own messages all stay in the main column.
 */
export function toThread(state: ChatState, agents: Agent[], unsent: SendFailure[], cache: Cache): Columns {
  const crew = team(state, new Map(agents.map((a) => [a.id, a.name])));
  const inTeam = (turn: string | null) => !!turn && !!crew.rootOf(turn);
  const main: ThreadMessageLike[] = [];
  const shown: Group[] = [];
  const work: Group[] = [];
  // A question to the person is the main column's, wherever it was asked: one asked in a team turn
  // leaves its line there, where that turn began (G160). Its answer line is the wake-up's, already there.
  const asked = new Set<string>();
  for (const g of grouped(state, inTeam)) {
    if (g.turn && !g.person && inTeam(g.turn) && !asked.has(g.turn)) {
      asked.add(g.turn);
      const line = questions(state, g.turn, crew, cache);
      if (line) main.push(line);
    }
    if (g.to) work.push(g);
    else if (g.person || !inTeam(g.turn)) {
      // What Gathr told the lead when a round settled: the team column and Summary already show it.
      // The person's own answers aren't shown there, so they keep their line.
      if (g.gathr && state.turns[g.turn ?? '']?.answers && !answered(state, g.turn).length) continue;
      main.push(cached(cache, g, state, crew));
      shown.push(g);
    } else work.push(g);
  }
  // Replies streaming at once save in the order they finish: the team keeps its turns' order, as a reload does.
  // Live updates can arrive out of order, so a turn's place wins; one not started yet goes last.
  const order = new Map(Object.keys(state.turns).map((id, i) => [id, i]));
  const place = (id: string | null) => state.turns[id ?? '']?.position ?? Number.POSITIVE_INFINITY;
  const seen = (id: string | null) => order.get(id ?? '') ?? 0;
  work.sort((a, b) => place(a.turn) - place(b.turn) || seen(a.turn) - seen(b.turn));
  // A send that opened a turn fails that turn too, and the turn's notice already says so.
  const told = new Set(Object.values(state.turns).map((t) => t.ref));
  for (const f of unsent) if (!f.failure.ref || !told.has(f.failure.ref)) main.push(failed(f));
  const find = (message: string): Spot | undefined => {
    const m = state.messages.find((x) => x.id === message);
    if (!m) return undefined;
    // A hear-back Gathr told the lead isn't drawn: its turn's reply stands for it.
    const sameTurn = (g: Group) => m.role === 'gathr' && !!m.turn && g.turn === m.turn;
    for (const [column, groups] of [
      ['main', shown],
      ['team', work],
    ] as const) {
      const g = groups.find((x) => x.msgs.includes(m)) ?? groups.find(sameTurn);
      if (!g) continue;
      const at = g.msgs.indexOf(m);
      if (!g.reply || at < 0) return { column, id: g.key };
      const nth = g.msgs.slice(0, at).filter((x) => x.kind !== 'tool' && x.body === m.body).length;
      return { column, id: g.key, text: m.body as string, nth };
    }
    return undefined;
  };
  return { main, team: work.map((g) => cached(cache, g, state, crew)), find };
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
      groups.push({ key: m.id, turn: m.turn, msgs: [m], reply: false, person: true });
      for (const turn of open.keys()) if (!inTeam(turn)) open.delete(turn);
    } else if (m.role === 'gathr') {
      if (!m.copy_of) groups.push({ key: m.id, turn: m.turn, msgs: [m], reply: false, gathr: true });
    } else if (m.to) {
      // Its own reply, so the sender's words before and after it stay one reply each.
      groups.push({ key: m.id, turn: m.turn, msgs: [m], reply: true, to: m.to });
      if (m.turn) open.delete(m.turn);
    } else if (m.turn && open.has(m.turn)) open.get(m.turn)?.msgs.push(m);
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
    // A message to a member never takes the turn's streaming text or notice.
    const at = groups.findLastIndex((g) => g.turn === t.id && !g.to);
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
  const heard = group.gathr ? answered(state, id) : [];
  const to = crew.name(group.to);
  const who = `${agent}|${role}|${to}|${JSON.stringify(task)}|${JSON.stringify(heard)}`;
  const asked = approvals.map((a) => `${a.request}:${a.answer?.allow}:${a.answer?.failure?.ref}`);
  const sig = `${parts}|${partial.length}|${ending}|${asked}|${who}`;
  const hit = cache.get(group.key);
  if (hit?.sig === sig) return hit.value;
  const value = group.gathr
    ? toGathr(group.msgs[0], heard.length ? { answered: heard } : task && { assignment: task })
    : group.reply
      ? toReply(group, partial, group.last ? turn : undefined, approvals, {
          agent,
          role,
          to,
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
  return {
    number: task.number,
    from: crew.name(task.giver),
    to: crew.name(task.owner),
    what: task.what,
    why: task.why ?? undefined,
  };
}

function toUser(m: Message): ThreadMessageLike {
  return {
    id: m.id,
    role: 'user',
    content: [{ type: 'text', text: m.body as string }],
    metadata: { custom: {} },
  };
}

/** The questions to the person this turn asked, as one quiet line each in the main column. */
function questions(state: ChatState, turn: string, crew: Team, cache: Cache): ThreadMessageLike | undefined {
  const lines = state.tasks
    .filter((t) => t.kind === 'person' && t.given_in === turn)
    .map((t) => ({
      line: `${crew.name(t.giver) ?? 'An agent'} asked you #${t.number} · ${t.what}`,
      why: t.why ?? undefined,
    }));
  if (!lines.length) return undefined;
  const key = `asked-${turn}`;
  const sig = JSON.stringify(lines);
  const hit = cache.get(key);
  if (hit?.sig === sig) return hit.value;
  const text = lines.map((l) => l.line).join('\n');
  const custom: Custom = { questions: lines };
  const value: ThreadMessageLike = {
    id: key,
    role: 'system',
    content: [{ type: 'text', text }],
    metadata: { custom },
  };
  cache.set(key, { sig, value });
  return value;
}

/** The person's answers that a hear-back in this turn told their asker, one line each. */
function answered(state: ChatState, turn: string | null): NonNullable<Custom['answered']> {
  if (!turn) return [];
  return state.tasks
    .filter((t) => t.kind === 'person' && t.state === 'done' && t.heard_in === turn)
    .map((t) => ({ line: `You answered #${t.number} · ${t.reason ?? 'Done'}`, asked: t.what }));
}

function toGathr(m: Message, line: Custom | undefined): ThreadMessageLike {
  const text = m.body as string;
  const custom: Custom = line ?? { gathr: text };
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
