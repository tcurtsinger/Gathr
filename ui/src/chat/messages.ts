import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Agent, Failure, Message, ToolBody, Turn } from '../api';
import type { Approval, ChatState } from './state';

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

/** Gathr's own part of a message, read by the thread's message components. */
export type Custom = {
  agent?: string;
  notice?: TurnNotice;
  approvals?: Approval[];
  /** A message that never reached the service. */
  unsent?: SendFailure;
};

type Part = Exclude<ThreadMessageLike['content'], string>[number];
type Cache = Map<string, { sig: string; value: ThreadMessageLike }>;
interface Group {
  key: string;
  turn: string | null;
  msgs: Message[];
  reply: boolean;
  /** The turn's last reply: the one its streaming text, notice and approvals go on. */
  last?: boolean;
}

const noticed = new Set(['stopped', 'waiting_sign_in', 'failed']);

/**
 * Gathr's chat as assistant-ui messages: each person's message, then the reply to it holding its
 * text and tool calls in order. A message sent into a running turn splits that turn's reply in two,
 * one before it and one after; the last holds the streaming text, the notice and the approvals.
 * A reply keeps one id from its first streamed text to its saved messages, so nothing remounts when
 * a stream ends, and a reply whose parts didn't change keeps the same object, so assistant-ui doesn't
 * convert it again.
 */
export function toThread(
  state: ChatState,
  agents: Agent[],
  unsent: SendFailure[],
  cache: Cache,
): ThreadMessageLike[] {
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

  const out = groups.map((group) => cached(cache, group, state, agents));
  // A send that opened a turn fails that turn too, and the turn's notice already says so.
  const told = new Set(Object.values(state.turns).map((t) => t.ref));
  for (const f of unsent) if (!f.failure.ref || !told.has(f.failure.ref)) out.push(failed(f));
  return out;
}

function cached(cache: Cache, group: Group, state: ChatState, agents: Agent[]) {
  const id = group.turn;
  const turn = id ? state.turns[id] : undefined;
  const partial = group.last && id ? (state.partial[id] ?? '') : '';
  const approvals = group.last ? state.approvals.filter((a) => a.turn === id) : [];
  const agent = agents.find((a) => a.id === turn?.agent)?.name;
  const parts = group.msgs.map((m) => `${m.id}:${m.kind === 'tool' ? (m.body as ToolBody).status : ''}`);
  const ending = group.last ? `${turn?.status}|${turn?.ref}` : '';
  const sig = `${parts}|${partial.length}|${ending}|${approvals.map((a) => a.request)}|${agent}`;
  const hit = cache.get(group.key);
  if (hit?.sig === sig) return hit.value;
  const value = group.reply
    ? toReply(group, partial, group.last ? turn : undefined, approvals, agent)
    : toUser(group.msgs[0]);
  cache.set(group.key, { sig, value });
  return value;
}

function toUser(m: Message): ThreadMessageLike {
  return { id: m.id, role: 'user', content: [{ type: 'text', text: m.body as string }] };
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
  agent: string | undefined,
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
  const custom: Custom = { agent, approvals };
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
    metadata: { custom: { ...custom } },
  };
}
