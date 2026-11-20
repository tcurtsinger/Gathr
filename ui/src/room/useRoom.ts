import { useCallback, useEffect, useRef, useState } from 'react';
import { api, type Failure, type History, type Member, type Message, type Reaction, type Turn } from '../api';
import { type LiveEvent, listen } from '../live';

/** What the room shows: its messages, turns, the words still streaming, its team and its thumbs-ups. */
export interface RoomState {
  messages: Message[];
  turns: Record<string, Turn>;
  /** Each running turn's words not saved yet. */
  partial: Record<string, string>;
  members: Member[];
  reactions: Reaction[];
}

export const emptyRoom: RoomState = { messages: [], turns: {}, partial: {}, members: [], reactions: [] };

export function fromHistory(h: History): RoomState {
  return {
    messages: h.messages,
    turns: Object.fromEntries(h.turns.map((t) => [t.id, t])),
    partial: Object.fromEntries((h.streaming ?? []).map((s) => [s.turn, s.text])),
    members: h.members ?? [],
    reactions: h.reactions ?? [],
  };
}

/** One live event for this chat. */
export function apply(s: RoomState, e: LiveEvent): RoomState {
  switch (e.type) {
    case 'delta': {
      const was = s.partial[e.turn] ?? '';
      const at = e.at ?? was.length;
      if (at + e.text.length <= was.length) return s;
      return { ...s, partial: { ...s.partial, [e.turn]: was + e.text.slice(Math.max(0, was.length - at)) } };
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
        why: e.why ?? before?.why ?? null,
        ref: e.ref ?? before?.ref ?? '',
        created: e.created ?? before?.created,
        ended: e.ended ?? before?.ended,
        topic: e.topic ?? before?.topic,
      };
      return { ...s, turns: { ...s.turns, [e.turn]: turn } };
    }
    case 'reaction': {
      const same = (r: Reaction) => r.message === e.reaction.message && r.agent === e.reaction.agent;
      const rest = s.reactions.filter((r) => !same(r));
      return { ...s, reactions: e.removed ? rest : [...rest, e.reaction] };
    }
    case 'members':
      return { ...s, members: e.members };
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
    held.current = [];
    api.history(chat).then(
      (h) => {
        const late = held.current ?? [];
        held.current = null;
        setState(late.reduce(apply, fromHistory(h)));
        setFailure(null);
      },
      (f: Failure) => {
        held.current = null;
        setFailure(f);
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

  return { state, failure, reload: load };
}
