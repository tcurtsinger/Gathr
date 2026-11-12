import type {
  Context,
  Member,
  Message,
  Pauses,
  PersonTask,
  StaleTask,
  Task,
  ToolBody,
  TurnStatus,
  Wait,
} from './api';

// Everything live from the service's one WebSocket (AGENTS.md "The API").
export type LiveEvent =
  // `at`: where the text starts in the turn's unsaved reply, in UTF-16 units.
  | { type: 'delta'; chat: string; turn: string; text: string; at?: number }
  | { type: 'message'; chat: string; message: Message }
  | { type: 'tool'; chat: string; message: string; body: ToolBody }
  // `agent`: the agent the turn was sent to, null for none.
  | {
      type: 'turn';
      chat: string;
      turn: string;
      status: TurnStatus;
      agent: string | null;
      why?: string | null;
      ref?: string;
      task?: number | null;
      answers?: string | null;
      held?: 'pause' | 'usage' | 'update' | 'claude' | 'codex' | null;
      created?: string;
      position?: number | null;
    }
  | { type: 'approval'; chat: string; turn: string; request_id: string; tool: string; input: unknown }
  | { type: 'read'; chat: string; turn: string | null; text: string }
  // The chat's tasks and waits, whole, after any change to them.
  | { type: 'tasks'; chat: string; tasks: Task[]; waits: Wait[] }
  // The person's open questions across chats, whole, after any change to any task.
  | { type: 'person_tasks'; tasks: PersonTask[]; stale: StaleTask[] }
  // A chat was made, archived, restored (unarchived) or deleted.
  | { type: 'chat'; chat: string; change: 'created' | 'archived' | 'unarchived' | 'deleted' }
  // A chat started or stopped working, or needing the person (0040): only when either changed.
  | { type: 'chat_state'; chat: string; working: boolean; needs: boolean }
  | { type: 'project'; project: string; change: 'created' | 'changed' }
  // A checks run of the project started or ended (0036 C3).
  | { type: 'checks'; project: string; head: string; run: string; state: 'running' | 'passed' | 'failed' }
  // The chat's team changed: an agent joined, or a PATCH.
  | { type: 'members'; chat: string; members: Member[] }
  // How full an agent's context is, after each model call or compaction.
  | { type: 'context'; chat: string; agent: string; context: Context }
  // Every pause and usage hold, whole, after any change: a pause, resume, Stop now, a pause going quiet.
  | ({ type: 'pauses' } & Pauses)
  // An account was added, renamed, or signed in or out: reload accounts.
  | { type: 'accounts' }
  // An agent was added or changed: reload agents.
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
