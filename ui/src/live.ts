import type {
  AppState,
  BoardLine,
  ChatMark,
  Context,
  Member,
  Message,
  Pauses,
  Question,
  Reaction,
  ThinkingBody,
  ToolBody,
  TurnEnd,
  TurnStatus,
} from './api';

// Everything live from the service's one WebSocket (AGENTS.md "The API").
export type LiveEvent =
  // `at`: where the text starts in the turn's unsaved reply, in UTF-16 units.
  // `reply_to`: in a room chat, the main message the words sit under once they start with REPLY.
  | { type: 'delta'; chat: string; turn: string; text: string; at?: number; reply_to?: string | null }
  | { type: 'message'; chat: string; message: Message }
  | { type: 'tool'; chat: string; message: string; body: ToolBody | ThinkingBody }
  // A new thumbs-up, an agent's or the person's like; a repeat sends nothing. `removed`: the person took theirs back.
  | { type: 'reaction'; chat: string; reaction: Reaction; removed?: boolean }
  // An agent set a line of the chat's board: the board, whole.
  | { type: 'board'; chat: string; lines: BoardLine[] }
  // `agent`: the agent the turn was sent to, null for none.
  | {
      type: 'turn';
      chat: string;
      turn: string;
      status: TurnStatus;
      agent: string | null;
      why?: string | null;
      end?: TurnEnd | null;
      ref?: string;
      task?: number | null;
      answers?: string | null;
      held?: 'pause' | 'usage' | 'update' | 'claude' | 'codex' | null;
      created?: string;
      /** When it ended, on its end (0082). */
      ended?: string | null;
      position?: number | null;
      topic?: string | null;
      result_of?: string | null;
      /** The kind of Gathr's words that opened it, as history carries it (0080). */
      opened?: string | null;
    }
  | { type: 'approval'; chat: string; turn: string; request_id: string; tool: string; input: unknown }
  | { type: 'read'; chat: string; turn: string | null; text: string }
  // A chat was made, archived, restored (unarchived) or deleted.
  | { type: 'chat'; chat: string; change: 'created' | 'archived' | 'unarchived' | 'deleted' }
  // A chat started or stopped working, needing the person (0040) or its row's mark (0071): only on a change.
  | { type: 'chat_state'; chat: string; working: boolean; needs: boolean; mark?: ChatMark | null }
  | { type: 'project'; project: string; change: 'created' | 'changed' | 'deleted' }
  // The chat's team changed: an agent joined, or a PATCH.
  | { type: 'members'; chat: string; members: Member[] }
  // A room chat's questions, whole, after every ask and answer.
  | { type: 'questions'; chat: string; questions: Question[] }
  // How full an agent's context is, after each model call or compaction.
  | { type: 'context'; chat: string; agent: string; context: Context }
  // Every pause and usage hold, whole, after any change: a pause, resume, Stop now, a pause going quiet.
  | ({ type: 'pauses' } & Pauses)
  // Gathr's own version and update, whole, after any change to it (0032).
  | ({ type: 'app' } & AppState)
  // An account was added, renamed, or signed in or out: reload accounts.
  | { type: 'accounts' }
  // An agent was added, changed or deleted: reload agents.
  | { type: 'agents' }
  // The client missed events (it fell behind, or the socket dropped): reload what it shows.
  | { type: 'lagged' };

type Listener = (event: LiveEvent) => void;

const listeners = new Set<Listener>();
let socket: WebSocket | null = null;
let retryMs = 250;
let missed = false;

function connect() {
  const url = `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/api/live`;
  const ws = new WebSocket(url);
  let dropped = false;
  ws.onopen = () => {
    if (missed) emit({ type: 'lagged' });
    missed = false;
    retryMs = 250;
  };
  ws.onmessage = (frame) => emit(JSON.parse(frame.data));
  ws.onclose = () => {
    if (dropped) return;
    dropped = true;
    socket = null;
    missed = true;
    if (listeners.size === 0) return;
    setTimeout(connect, retryMs);
    retryMs = Math.min(retryMs * 2, 5000);
  };
  socket = ws;
}

function emit(event: LiveEvent) {
  for (const listener of listeners) listener(event);
}

/** Calls `listener` with every live event until the returned function is called. */
export function listen(listener: Listener): () => void {
  listeners.add(listener);
  if (!socket) connect();
  return () => listeners.delete(listener);
}
