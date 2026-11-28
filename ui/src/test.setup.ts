// Every test gets the app's styles, as the app entry does: without them WebKit shows a registry tooltip
// unstyled, in the flow, where it can cover the menu item a test clicks next (archive.test.tsx did).
import { beforeAll } from 'vitest';
import './theme.css';
import './test.frames';

// The font arrives by swap, whenever the file does: a swap mid-test moves the text a test measures.
// beforeAll runs after the imports, so the font face is known.
beforeAll(async () => {
  await document.fonts.load('15px "Hanken Grotesk Variable"');
});

// The tests' server answers none of the service's sockets: one asked for stays connecting for good (checked in
// Chromium). Since every chat opened its terminals' and browser's sockets (#477), WebKit on the Mac stopped connecting
// Vitest's own to the next test file ("Cannot connect to the server in 60 seconds"). Here each closes at once, as
// with no service running.
class NoService extends EventTarget {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  readyState = 3;
  binaryType = 'blob';
  onopen = null;
  onmessage = null;
  onerror = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  constructor(readonly url: string) {
    super();
    setTimeout(() => this.onclose?.(new CloseEvent('close', { code: 1006 })));
  }
  send() {}
  close() {}
}
globalThis.WebSocket = NoService as unknown as typeof WebSocket;
