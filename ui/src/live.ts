import type { Member, Message, Task, ToolBody, TurnStatus, Wait } from './api';

// Everything live from the service's one WebSocket (AGENTS.md "The API").
export type LiveEvent =
  | { type: 'delta'; chat: string; turn: string; text: string }
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
    }
  | { type: 'approval'; chat: string; turn: string; request_id: string; tool: string; input: unknown }
  | { type: 'read'; chat: string; turn: string | null; text: string }
  // The chat's tasks and waits, whole, after any change to them.
  | { type: 'tasks'; chat: string; tasks: Task[]; waits: Wait[] }
  // The chat's team changed: an agent joined, or a PATCH.
  | { type: 'members'; chat: string; members: Member[] }
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
