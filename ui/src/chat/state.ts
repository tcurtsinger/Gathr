import type { History, Message, ToolBody, Turn } from '../api';
import type { LiveEvent } from '../live';

export interface Approval {
  turn: string;
  request: string;
  tool: string;
}

/** One chat as the window shows it: saved messages, turns, and text still streaming. */
export interface ChatState {
  messages: Message[];
  turns: Record<string, Turn>;
  partial: Record<string, string>;
  approvals: Approval[];
}

export const empty: ChatState = { messages: [], turns: {}, partial: {}, approvals: [] };

export function fromHistory(history: History): ChatState {
  const turns = Object.fromEntries(history.turns.map((t) => [t.id, t]));
  const approvals = history.approvals.map((a) => ({ turn: a.turn, request: a.request_id, tool: a.tool }));
  return { ...empty, messages: history.messages, turns, approvals };
}

/** Applies one live event for this chat. Events for other chats never reach here. */
export function apply(state: ChatState, event: LiveEvent): ChatState {
  switch (event.type) {
    case 'delta': {
      const text = (state.partial[event.turn] ?? '') + event.text;
      return { ...state, partial: { ...state.partial, [event.turn]: text } };
    }
    case 'message': {
      const m = event.message;
      const at = state.messages.findIndex((x) => x.id === m.id);
      const messages = at < 0 ? [...state.messages, m] : state.messages.with(at, m);
      // A saved text message is the text that was streaming; the stream starts over after it.
      const saved = m.role === 'assistant' && m.kind === 'text' && m.turn;
      const partial = saved ? without(state.partial, m.turn as string) : state.partial;
      return { ...state, messages, partial };
    }
    case 'tool': {
      const messages = state.messages.map((m) =>
        m.id === event.message ? { ...m, body: event.body as ToolBody } : m,
      );
      return { ...state, messages };
    }
    case 'turn': {
      const before = state.turns[event.turn];
      const turn: Turn = {
        id: event.turn,
        status: event.status,
        why: event.why ?? before?.why ?? null,
        ref: event.ref ?? before?.ref ?? '',
      };
      const open = event.status === 'running' || event.status === 'queued';
      const approvals = open ? state.approvals : state.approvals.filter((a) => a.turn !== event.turn);
      return { ...state, turns: { ...state.turns, [event.turn]: turn }, approvals };
    }
    case 'approval': {
      const approval = { turn: event.turn, request: event.request_id, tool: event.tool };
      return { ...state, approvals: [...state.approvals, approval] };
    }
    default:
      return state;
  }
}

export function answered(state: ChatState, request: string): ChatState {
  return { ...state, approvals: state.approvals.filter((a) => a.request !== request) };
}

/** The turn still under way, if any: Stop applies to it. */
export function running(state: ChatState): Turn | undefined {
  return Object.values(state.turns).find((t) => t.status === 'running' || t.status === 'queued');
}

function without(record: Record<string, string>, key: string): Record<string, string> {
  const { [key]: _, ...rest } = record;
  return rest;
}
