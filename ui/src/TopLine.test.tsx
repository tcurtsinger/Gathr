// The window with no system title bar (0070): the top line moves the window and, on Windows, holds its three
// buttons. The system's edge is stubbed: Tauri's IPC, and the user agent that says which system the page is on.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { App } from './App';
import type { LiveEvent } from './live';
import { TopLine } from './TopLine';

vi.mock('./live', () => ({ listen: (_: (event: LiveEvent) => void) => () => {} }));

const WINDOWS =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 Edg/142.0.0.0';
const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)';

/** The page on `system`; in the desktop window when `window` is given, which answers each command it is sent. */
function on(system: string, window?: (command: string) => unknown) {
  vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue(system);
  const sent: string[] = [];
  if (window) {
    vi.stubGlobal('__TAURI_INTERNALS__', {
      invoke: async (command: string) => {
        sent.push(command);
        return window(command);
      },
    });
  }
  return sent;
}
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const json = (body: unknown) => new Response(JSON.stringify(body));
/** The service, with `chats` and nothing else. */
function service(chats: { id: string; title: string }[]) {
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/chats') return json(chats);
    if (path === '/api/chats?archived=true') return json([]);
    if (path === '/api/agents') return json({ agents: [], removed: [] });
    if (path === '/api/projects') return json({ projects: [] });
    if (path === '/api/person-tasks') return json({ tasks: [], stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    return json({});
  });
}

test('on Windows the top line minimizes, maximizes and closes the window', async () => {
  const sent = on(WINDOWS, (command) => (command === 'plugin:window|is_maximized' ? false : null));
  const screen = await render(<TopLine />);
  await screen.getByRole('button', { name: 'Minimize' }).click();
  await screen.getByRole('button', { name: 'Maximize' }).click();
  await screen.getByRole('button', { name: 'Close' }).click();
  expect(sent.filter((c) => c !== 'plugin:window|is_maximized')).toEqual([
    'plugin:window|minimize',
    'plugin:window|toggle_maximize',
    'plugin:window|close',
  ]);
});

test('the maximize button reads Restore while the window is maximized, and follows it', async () => {
  let maximized = true;
  on(WINDOWS, (command) => (command === 'plugin:window|is_maximized' ? maximized : null));
  const screen = await render(<TopLine />);
  await expect.element(screen.getByRole('button', { name: 'Restore' })).toBeVisible();
  // Snapped, dragged down or restored by the system: the window resizes, and the button reads it again.
  maximized = false;
  window.dispatchEvent(new Event('resize'));
  await expect.element(screen.getByRole('button', { name: 'Maximize' })).toBeVisible();
});

test('in a browser the top line draws no window buttons and moves nothing', async () => {
  on(WINDOWS);
  const screen = await render(<TopLine />);
  const drawn = [...screen.container.querySelectorAll('button')].map((b) => b.getAttribute('aria-label'));
  expect(drawn).toEqual([]);
  expect(screen.container.querySelector('header')?.getAttribute('data-tauri-drag-region')).toBeNull();
});

/** What a press at each point lands on: the drag region it starts in, or the element when it starts none. */
const pressed = (points: [number, number][]) =>
  points.map(([x, y]) => {
    const at = document.elementFromPoint(x, y);
    const region = at?.closest('[data-tauri-drag-region="deep"]');
    return region && at === region ? region.className : `${at?.tagName}.${at?.className}`;
  });

test('on the Mac the top line draws no window buttons and leaves the system’s three empty, to move the window', async () => {
  on(MAC, () => false);
  service([{ id: 'c', title: 'Made-up chat' }]);
  await page.viewport(1440, 900);
  const screen = await render(<App />);
  await expect.element(screen.getByRole('heading', { name: 'Made-up chat' })).toBeVisible();
  for (const name of ['Minimize', 'Maximize', 'Restore', 'Close']) {
    expect(screen.getByRole('button', { name, exact: true }).elements(), name).toEqual([]);
  }
  // The mock-up's Mac case: the system's buttons 12px round, 8px apart, 14px in from the left and the top.
  expect(
    pressed([
      [20, 20],
      [40, 20],
      [60, 20],
    ]),
  ).toEqual(['topline', 'topline', 'topline']);
});

test('with no chat open the window still has its top line, to move it and close it', async () => {
  const sent = on(WINDOWS, () => false);
  service([]);
  await page.viewport(1440, 900);
  const screen = await render(<App />);
  await expect.element(screen.getByText('No chats yet')).toBeVisible();
  expect(pressed([[700, 20]]), 'a press on the top line, over the chat').toEqual(['topline with-buttons']);
  await screen.getByRole('button', { name: 'Close' }).click();
  expect(sent).toContain('plugin:window|close');
});

test('a window command the window refuses goes to the event record, named for the command', async () => {
  on(WINDOWS, (command) => {
    if (command === 'plugin:window|close') throw new Error('made-up refusal');
    return false;
  });
  const reported: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/ui/events') reported.push(JSON.parse(String(init?.body)));
    return json({});
  });
  const screen = await render(<TopLine />);
  await screen.getByRole('button', { name: 'Close' }).click();
  await expect
    .poll(() => reported)
    .toEqual([
      { name: 'gathr.ui.window.frame', error: 'close', attrs: { message: 'Error: made-up refusal' } },
    ]);
});
