import { useCallback, useEffect, useRef, useState } from 'react';
import {
  api,
  type Background,
  type Failure,
  type History,
  type Member,
  type Message,
  type Question,
  type Reaction,
  type Stay,
  type Turn,
} from '../api';
import type { Approval } from '../components/Approval';
import { type LiveEvent, listen } from '../live';

/** What the room shows: its messages, turns, the words still streaming, its team, thumbs-ups, approvals, questions. */
export interface RoomState {
  messages: Message[];
  turns: Record<string, Turn>;
  /** Each running turn's words not saved yet. */
  partial: Record<string, string>;
  /** The main message each streaming reply sits under, before its words are saved. */
  under: Record<string, string>;
  members: Member[];
  reactions: Reaction[];
  approvals: Approval[];
  questions: Question[];
  /** What its agents' engines still run in the background. */
  background: Background[];
  /** In the assistant's chat, who joined and left, and when. */
  stays: Stay[];
}

export const emptyRoom: RoomState = {
  messages: [],
  turns: {},
  partial: {},
  under: {},
  members: [],
  reactions: [],
  approvals: [],
  questions: [],
  background: [],
  stays: [],
};

export function fromHistory(h: History): RoomState {
  return {
    messages: h.messages,
    turns: Object.fromEntries(h.turns.map((t) => [t.id, t])),
    partial: Object.fromEntries((h.streaming ?? []).map((s) => [s.turn, s.text])),
    under: Object.fromEntries((h.streaming ?? []).flatMap((s) => (s.reply_to ? [[s.turn, s.reply_to]] : []))),
    members: h.members ?? [],
    reactions: h.reactions ?? [],
    approvals: h.approvals.map((a) => ({
      turn: a.turn,
      request: a.request_id,
      tool: a.tool,
      input: a.input,
      why: a.why,
    })),
    questions: h.questions ?? [],
    background: h.background ?? [],
    stays: h.stays ?? [],
  };
}

/** One live event for this chat. */
export function apply(s: RoomState, e: LiveEvent): RoomState {
  switch (e.type) {
    case 'delta': {
      const was = s.partial[e.turn] ?? '';
      const at = e.at ?? was.length;
      if (at + e.text.length <= was.length) return s;
      // Each block of words says where it sits: a turn's last one decides.
      const { [e.turn]: _, ...rest } = s.under;
      const under = e.reply_to ? { ...rest, [e.turn]: e.reply_to } : rest;
      return {
        ...s,
        under,
        partial: { ...s.partial, [e.turn]: was + e.text.slice(Math.max(0, was.length - at)) },
      };
    }
    case 'message': {
      const m = e.message;
      const i = s.messages.findIndex((x) => x.id === m.id);
      const messages = i < 0 ? [...s.messages, m] : s.messages.with(i, m);
      // Saved words are the words that were streaming; the stream starts over after them.
      if (m.role !== 'assistant' || m.kind !== 'text' || !m.turn) return { ...s, messages };
      const { [m.turn]: _, ...partial } = s.partial;
      return { ...s, messages, partial };
    }
    case 'tool':
      return { ...s, messages: s.messages.map((m) => (m.id === e.message ? { ...m, body: e.body } : m)) };
    case 'turn': {
      const before = s.turns[e.turn];
      const turn: Turn = {
        ...before,
        id: e.turn,
        status: e.status,
        agent: e.agent,
        // A null why clears the one before: a hold let go, a turn that started.
        why: e.why === undefined ? (before?.why ?? null) : e.why,
        ref: e.ref ?? before?.ref ?? '',
        created: e.created ?? before?.created,
        ended: e.ended ?? before?.ended,
        opened: e.opened ?? before?.opened,
      };
      // A turn that ended waits on nothing.
      const open = e.status === 'running' || e.status === 'queued';
      const approvals = open ? s.approvals : s.approvals.filter((a) => a.turn !== e.turn);
      return { ...s, turns: { ...s.turns, [e.turn]: turn }, approvals };
    }
    case 'approval': {
      const asked = { turn: e.turn, request: e.request_id, tool: e.tool, input: e.input, why: e.why };
      // The history may hold it already.
      return { ...s, approvals: [...s.approvals.filter((a) => a.request !== e.request_id), asked] };
    }
    case 'reaction': {
      const same = (r: Reaction) => r.message === e.reaction.message && r.agent === e.reaction.agent;
      const rest = s.reactions.filter((r) => !same(r));
      return { ...s, reactions: e.removed ? rest : [...rest, e.reaction] };
    }
    case 'members': {
      // The event names the team only: each agent keeps the context it had.
      const had = (m: Member) => s.members.find((x) => x.agent === m.agent)?.context;
      return { ...s, members: e.members.map((m) => ({ ...m, context: m.context ?? had(m) })) };
    }
    case 'questions':
      return { ...s, questions: e.questions };
    case 'context':
      return {
        ...s,
        members: s.members.map((m) => (m.agent === e.agent ? { ...m, context: e.context } : m)),
      };
    case 'background':
      return { ...s, background: e.background };
    case 'stay':
      // One that came while the history loaded is in the history too.
      return s.stays.some((st) => st.agent === e.stay.agent && st.time === e.stay.time)
        ? s
        : { ...s, stays: [...s.stays, e.stay] };
    default:
      return s;
  }
}

/** One chat's room: its history, kept current from the live socket. */
export function useRoom(chat: string) {
  const [state, setState] = useState<RoomState>(emptyRoom);
  const [failure, setFailure] = useState<Failure | null>(null);

  // Events that come while the history loads, applied on top of it once it is in.
  const held = useRef<LiveEvent[] | null>(null);

  const load = useCallback(() => {
    // Each load holds its own events; one a newer load started over is dropped, as its history may be the older.
    const late: LiveEvent[] = [];
    held.current = late;
    api.history(chat).then(
      (h) => {
        if (held.current !== late) return;
        held.current = null;
        setState(late.reduce(apply, fromHistory(h)));
        setFailure(null);
      },
      (f: Failure) => {
        if (held.current !== late) return;
        held.current = null;
        if (!f.away) setFailure(f);
      },
    );
  }, [chat]);

  useEffect(() => {
    setState(emptyRoom);
    // A new chat the service hasn't made yet has nothing to load.
    if (!chat) return;
    load();
    return listen((e) => {
      if (e.type === 'lagged') return load();
      if (!('chat' in e) || e.chat !== chat) return;
      if (held.current) held.current.push(e);
      else setState((s) => apply(s, e));
    });
  }, [chat, load]);

  // The card stays until the service takes the answer; a refusal shows on it, to answer again.
  const approve = useCallback(
    (request: string, allow: boolean) => {
      const answer = (to?: Approval['answer']) => (s: RoomState) => ({
        ...s,
        approvals: s.approvals.flatMap((a) =>
          a.request !== request ? [a] : to ? [{ ...a, answer: to }] : [],
        ),
      });
      setState(answer({ allow }));
      api.approve(chat, request, allow).then(
        () => setState(answer()),
        (failure: Failure) => setState(answer({ allow, failure })),
      );
    },
    [chat],
  );

  // The answer shows at once; the live event brings the same.
  const answer = useCallback(
    async (asked: Question, pick?: string, words?: string, files?: readonly File[]) => {
      const { question } = await api.answerQuestion(asked, pick, words, files);
      setState((s) => ({ ...s, questions: s.questions.map((q) => (q.id === asked.id ? question : q)) }));
    },
    [],
  );

  return { state, failure, reload: load, approve, answer };
}
