import { useSyncExternalStore } from 'react';

/** One page of the person's browser, which the service runs without a window (docs/api.md, the Browser tab). */
export interface BrowserPage {
  id: string;
  title: string;
  url: string;
  loading: boolean;
  back: boolean;
  forward: boolean;
}

export interface BrowserState {
  running: boolean;
  /** No browser on this computer: what to install. */
  missing: string | null;
  pages: BrowserPage[];
  shown: string | null;
  /** One plain line about what the page tried and isn't built yet. */
  line: string | null;
}

const NONE: BrowserState = { running: false, missing: null, pages: [], shown: null, line: null };
let state = NONE;
const heard = new Set<() => void>();
const drawn = new Set<(frame: Blob) => void>();
let socket: WebSocket | null = null;
let retryMs = 250;
/** What the tab last said of itself, said again after the socket comes back. */
let view: object | null = null;
/** What is selected in the shown page, for the window's own copy. */
let selection = '';
export const selected = () => selection;

function connect() {
  const ws = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/api/browser`);
  ws.binaryType = 'blob';
  ws.onopen = () => {
    retryMs = 250;
    if (view) ws.send(JSON.stringify(view));
  };
  ws.onmessage = (m) => {
    if (m.data instanceof Blob) {
      for (const draw of drawn) draw(m.data);
      return;
    }
    const said = JSON.parse(m.data);
    if (said.type === 'selection') selection = said.text;
    if (said.type !== 'state') return;
    state = said;
    for (const heed of heard) heed();
  };
  ws.onclose = () => {
    if (socket !== ws) return;
    socket = null;
    setTimeout(connect, retryMs);
    retryMs = Math.min(retryMs * 2, 5000);
  };
  socket = ws;
}

function listen(heed: () => void) {
  heard.add(heed);
  if (!socket) connect();
  return () => heard.delete(heed);
}

/** The browser's tabs as the service has them; the socket opens on first use and stays open. */
export const useBrowser = () => useSyncExternalStore(listen, () => state);

/** Tells the service what the person did, or what the tab is (`view`: its size and whether it shows). */
export function say(message: { type: string } & Record<string, unknown>) {
  if (message.type === 'view') view = message;
  if (!socket) connect();
  if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
}

/** Calls `draw` with each frame of the shown page's live picture, a JPEG. */
export function onFrame(draw: (frame: Blob) => void) {
  drawn.add(draw);
  return () => {
    drawn.delete(draw);
  };
}

/** A page's name in its tab: its title, else its site. */
export function pageName(page: BrowserPage) {
  if (page.title && page.title !== page.url) return page.title;
  try {
    return new URL(page.url).host || 'New page';
  } catch {
    return 'New page';
  }
}
