// A narrow window never covers the chat (DESIGN.md Layout): the workspace sits beside the chat at every width
// from the window's smallest, 960, and the chat keeps 360. Measured in the real layout, with the app's styles.
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { App } from './App';
import type { LiveEvent } from './live';

vi.mock('./live', () => ({ listen: (_: (event: LiveEvent) => void) => () => {} }));

const json = (body: unknown) => new Response(JSON.stringify(body));
const history = {
  chat: 'c',
  members: [{ agent: 'mira', name: 'Made-up Mira', role: 'supervisor' }],
  approvals: [],
  turns: [{ id: 't', status: 'done', why: null, ref: 'a', agent: 'mira' }],
  messages: [
    { id: 'm1', turn: 't', time: '2026-11-11T10:00:00Z', role: 'user', kind: 'text', body: 'Made-up ask' },
    {
      id: 'm2',
      turn: 't',
      time: '2026-11-11T10:00:30Z',
      role: 'assistant',
      kind: 'text',
      body: 'Made-up answer',
    },
  ],
};

function service() {
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/chats') return json([{ id: 'c', title: 'Made-up chat' }]);
    if (path === '/api/chats?archived=true') return json([]);
    if (path === '/api/agents') return json({ agents: [] });
    if (path === '/api/projects') return json({ projects: [] });
    if (path === '/api/person-tasks') return json({ tasks: [], stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    if (path === '/api/chats/c/tasks') return json({ tasks: [], waits: [] });
    if (path === '/api/chats/c') return json(history);
    return json({});
  });
}

const forget = () => {
  // biome-ignore lint/suspicious/noDocumentCookie: the width the handle keeps, from an earlier test.
  document.cookie = 'gathr.width--workspace-width=; path=/; max-age=0';
  document.documentElement.style.removeProperty('--workspace-width');
};
beforeEach(forget);
afterEach(() => {
  vi.unstubAllGlobals();
  forget();
});

const width = (selector: string) => {
  const box = document.querySelector(selector)?.getBoundingClientRect();
  return box
    ? { left: Math.round(box.left), right: Math.round(box.right), width: Math.round(box.width) }
    : null;
};

/** The three columns as drawn, so a failure prints what was measured. */
const columns = () => ({
  window: innerWidth,
  sidebar: width('.sidebar'),
  chat: width('main.chat'),
  panel: width('aside.workspace'),
});

async function openPanel() {
  service();
  await render(<App />);
  await page.getByRole('button', { name: 'Made-up chat', exact: true }).click();
  await page.getByRole('button', { name: 'Open sidebar' }).click();
  await expect.element(page.getByRole('complementary', { name: 'Chat workspace' })).toBeVisible();
}

/** Beside, not over: the chat at least 360, the panel at least 360 and after the chat, all inside the window. */
function expectBeside(at: ReturnType<typeof columns>) {
  const { sidebar, chat, panel } = at;
  expect(chat && panel && sidebar, JSON.stringify(at)).toBeTruthy();
  if (!chat || !panel || !sidebar) return;
  expect(chat.width, `the chat column, of ${JSON.stringify(at)}`).toBeGreaterThanOrEqual(360);
  expect(panel.width, `the panel, of ${JSON.stringify(at)}`).toBeGreaterThanOrEqual(360);
  expect(
    panel.left,
    `the panel's left edge against the chat's right, of ${JSON.stringify(at)}`,
  ).toBeGreaterThanOrEqual(chat.right);
  expect(
    panel.right,
    `the panel's right edge against the window, of ${JSON.stringify(at)}`,
  ).toBeLessThanOrEqual(at.window);
  expect(sidebar.width, `the sidebar, of ${JSON.stringify(at)}`).toBeGreaterThanOrEqual(190);
}

test.each([1000, 960])('at %i wide the open panel sits beside the chat, which keeps 360', async (w) => {
  await page.viewport(w, 700);
  await openPanel();
  expectBeside(columns());
});

test('a width the person dragged gives way in a narrow window and comes back when it is wide again', async () => {
  await page.viewport(1440, 900);
  await openPanel();
  const handle = page.getByRole('separator', { name: 'Resize chat sidebar' });
  await handle.click();
  // The handle's keys set the width as a drag does: 200px wider.
  await userEvent.keyboard('{Shift>}{ArrowLeft}{ArrowLeft}{ArrowLeft}{ArrowLeft}{ArrowLeft}{/Shift}');
  const dragged = columns().panel?.width ?? 0;
  expect(dragged, JSON.stringify(columns())).toBeGreaterThan(600);

  await page.viewport(1000, 700);
  await expect
    .poll(() => columns().panel?.width, {
      message: `the panel at 1000, narrower than the dragged ${dragged}`,
    })
    .toBeLessThan(dragged);
  expectBeside(columns());

  await page.viewport(1440, 900);
  await expect
    .poll(() => columns().panel?.width, { message: `the panel back at 1440, at the dragged ${dragged}` })
    .toBe(dragged);
});
