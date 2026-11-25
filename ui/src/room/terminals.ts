import { useSyncExternalStore } from 'react';

/** One of a chat's terminals, which the service runs (docs/api.md, the Terminal tab). */
export interface TerminalInfo {
  id: string;
  chat: string;
  /** "Terminal", or the command an agent ran. */
  name: string;
  folder: string;
  /** The agent that started it; none for the person's. */
  agent: string | null;
  command: string | null;
  running: boolean;
  code: number | null;
}

/** What one chat's socket holds: its terminals, what each printed, and who is listening. */
interface Chat {
  socket: WebSocket | null;
  state: { list: TerminalInfo[]; failed: string | null };
  printed: Map<string, Uint8Array[]>;
  heard: Set<() => void>;
  shown: Map<string, Set<Write>>;
  retryMs: number;
  subscribe?: (heed: () => void) => () => void;
}

/** About as much as the service keeps of each, so a tab opened late shows what it would. */
const KEPT = 1 << 20;
const ID = 26;
const chats = new Map<string, Chat>();
const decoder = new TextDecoder();
const NONE = { list: [], failed: null };
/** A piece a terminal printed, or null: start over, the whole of it comes again. */
type Write = (bytes: Uint8Array | null) => void;

/** Everything shown is about to come again from the start. */
function restart(c: Chat) {
  c.printed = new Map();
  for (const set of c.shown.values()) for (const write of set) write(null);
}

function chatOf(chat: string): Chat {
  let c = chats.get(chat);
  if (!c) {
    c = { socket: null, state: NONE, printed: new Map(), heard: new Set(), shown: new Map(), retryMs: 250 };
    chats.set(chat, c);
  }
  return c;
}

const heed = (c: Chat) => {
  for (const h of c.heard) h();
};

function connect(chat: string) {
  const c = chatOf(chat);
  const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
  const ws = new WebSocket(`${scheme}://${location.host}/api/chats/${encodeURIComponent(chat)}/terminals`);
  ws.binaryType = 'arraybuffer';
  ws.onopen = () => {
    c.retryMs = 250;
    restart(c);
  };
  ws.onmessage = (m) => {
    if (m.data instanceof ArrayBuffer) {
      const id = decoder.decode(new Uint8Array(m.data, 0, ID));
      const bytes = new Uint8Array(m.data, ID);
      if (!bytes.length) return;
      const kept = c.printed.get(id) ?? [];
      kept.push(bytes);
      let size = kept.reduce((n, b) => n + b.length, 0);
      while (size > KEPT && kept.length > 1) size -= kept.shift()?.length ?? 0;
      c.printed.set(id, kept);
      for (const write of c.shown.get(id) ?? []) write(bytes);
      return;
    }
    const said = JSON.parse(m.data);
    if (said.type === 'again') restart(c);
    if (said.type === 'failed') c.state = { ...c.state, failed: said.why };
    if (said.type === 'list') c.state = { list: said.terminals, failed: null };
    heed(c);
  };
  ws.onclose = () => {
    if (c.socket !== ws) return;
    c.socket = null;
    if (!c.heard.size) return;
    setTimeout(() => c.heard.size && !c.socket && connect(chat), c.retryMs);
    c.retryMs = Math.min(c.retryMs * 2, 5000);
  };
  c.socket = ws;
}

/** Says something to the service about this chat's terminals. */
export function tell(chat: string, said: object) {
  const ws = chatOf(chat).socket;
  if (ws?.readyState === WebSocket.OPEN) ws.send(JSON.stringify(said));
}

/** What a terminal printed so far, then each new piece as it comes. */
export function follow(chat: string, id: string, write: Write) {
  const c = chatOf(chat);
  for (const bytes of c.printed.get(id) ?? []) write(bytes);
  const set = c.shown.get(id) ?? new Set();
  set.add(write);
  c.shown.set(id, set);
  return () => set.delete(write);
}

/** The chat's terminals, oldest first, and why the last ask failed; the socket is open while anything shows them. */
export function useTerminals(chat: string) {
  const c = chatOf(chat);
  c.subscribe ??= (h) => {
    c.heard.add(h);
    if (!c.socket) connect(chat);
    return () => {
      c.heard.delete(h);
      // A moment later, so a view that only moved keeps the socket.
      setTimeout(() => {
        if (c.heard.size || !c.socket) return;
        const ws = c.socket;
        c.socket = null;
        ws.close();
      }, 1000);
    };
  };
  return useSyncExternalStore(c.subscribe, () => c.state);
}

/** Chats whose person asked for a new terminal: it shows once it comes. */
export const asked = new Set<string>();
/** Of those, the chats whose new terminal has a command typed in, waiting for the person's Enter. */
export const waiting = new Set<string>();

/**
 * Starts the person's terminal in `folder`, or the chat's first, and shows it; with `typed`, a command an agent asked
 * them to run, typed in and run at once, or left for their Enter.
 */
export function openTerminal(chat: string, folder?: string, typed?: string, run?: boolean) {
  asked.add(chat);
  if (typed && !run) waiting.add(chat);
  else waiting.delete(chat);
  tell(chat, { type: 'open', folder, typed, run });
}
