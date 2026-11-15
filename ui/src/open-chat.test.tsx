// A click on a system notification opens its chat (0058): the desktop app calls the page's
// `gathrOpenChat`. The service is the edge: fetch answers as it would.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { App } from './App';
import type { LiveEvent } from './live';

vi.mock('./live', () => ({ listen: (_: (event: LiveEvent) => void) => () => {} }));
afterEach(() => vi.unstubAllGlobals());

const json = (body: unknown) => new Response(JSON.stringify(body));
const history = (chat: string) => ({ chat, members: [], approvals: [], turns: [], messages: [] });

test("a notification's click opens its chat, and one for a chat that's gone is recorded", async () => {
  const reported: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/ui/events') reported.push(JSON.parse(String(init?.body)));
    if (path === '/api/chats')
      return json([
        { id: 'c', title: 'Made-up round' },
        { id: 'd', title: 'Fix the login test' },
      ]);
    if (path === '/api/chats?archived=true') return json([]);
    if (path === '/api/agents') return json({ agents: [] });
    if (path === '/api/projects') return json({ projects: [] });
    if (path === '/api/person-tasks') return json({ tasks: [], stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    if (path === '/api/chats/c' || path === '/api/chats/d') return json(history(path.slice(11)));
    return json({});
  });
  await render(<App />);
  const title = page.getByRole('heading', { level: 1 });
  await expect.element(title).toHaveTextContent('Made-up round');

  const open = (chat: string) => (window as { gathrOpenChat?: (chat: string) => void }).gathrOpenChat?.(chat);
  open('d');
  await expect.element(title).toHaveTextContent('Fix the login test');

  open('nope');
  await expect
    .poll(() => reported.filter((e) => (e as { name: string }).name === 'gathr.ui.notify'))
    .toEqual([{ name: 'gathr.ui.notify', error: 'gone', attrs: { outcome: 'clicked', chat: 'nope' } }]);
  await expect.element(title).toHaveTextContent('Fix the login test');
});
