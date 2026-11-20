// The window itself never scrolls (app.css): the app fills it and only its panes scroll, so a box that reaches
// past the app, wherever it is placed from, gives the page nothing to scroll.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { App } from './App';
import type { LiveEvent } from './live';

vi.mock('./live', () => ({ listen: (_: (event: LiveEvent) => void) => () => {} }));

const json = (body: unknown) => new Response(JSON.stringify(body));
const strays: HTMLElement[] = [];
afterEach(() => {
  vi.unstubAllGlobals();
  for (const s of strays.splice(0)) s.remove();
  scrollTo(0, 0);
});

/** A 1px box far below the window, placed from `inside`: as a hidden status line deep in a long pane is. */
function stray(inside: Element) {
  const box = document.createElement('div');
  box.style.cssText = 'position:absolute;top:3000px;left:0;width:1px;height:1px';
  inside.append(box);
  strays.push(box);
}

test('a box that reaches far below the app leaves the window nothing to scroll', async () => {
  await page.viewport(1200, 700);
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/chats' || path === '/api/chats?archived=true') return json([]);
    if (path === '/api/agents') return json({ agents: [], removed: [] });
    if (path === '/api/projects') return json({ projects: [] });
    if (path.endsWith('/board')) return json({ lines: [], agents: 1 });
    if (path === '/api/person-tasks') return json({ tasks: [], stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    return json({});
  });
  await render(<App />);
  await expect.element(page.getByRole('button', { name: 'Settings' })).toBeVisible();
  const app = document.querySelector('.app') as HTMLElement;

  // One placed from the app itself, one from the page: a menu or card drawn outside the app is placed there.
  stray(app);
  stray(document.body);
  const drawn = () => ({ page: document.documentElement.scrollHeight, window: innerHeight });
  expect(drawn(), JSON.stringify(drawn())).toEqual({ page: 700, window: 700 });
  scrollTo(0, 500);
  document.body.scrollTop = 500;
  expect([scrollY, document.body.scrollTop], 'the window and the page after a scroll by script').toEqual([
    0, 0,
  ]);
  expect(Math.round(app.getBoundingClientRect().top), "the app's top edge").toBe(0);
});
