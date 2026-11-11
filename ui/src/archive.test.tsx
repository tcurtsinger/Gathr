// Archive, Restore and Delete across the app (F11). The service is the edge: fetch answers as it would.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { App } from './App';
import type { Chat } from './api';
import type { LiveEvent } from './live';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('./live', () => ({
  listen: (on: (event: LiveEvent) => void) => {
    live.add(on);
    return () => live.delete(on);
  },
}));
afterEach(() => vi.unstubAllGlobals());

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });
const refusal = (error: string) => json({ error, why: 'made-up refusal', ref: 'made-up-ref' }, 409);

/** The service with these chats; `answer` settles archive, unarchive and delete, else they succeed. */
function service(active: Chat[], archived: Chat[], answer?: (path: string) => Response | undefined) {
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    const method = init?.method ?? 'GET';
    const [, id, action] = path.match(/^\/api\/chats\/([^/?]+)\/?(\w*)$/) ?? [];
    if (method !== 'GET' && id && (method === 'DELETE' || action === 'archive' || action === 'unarchive')) {
      const refused = answer?.(path);
      if (refused) return refused;
      const chat = [...active, ...archived].find((c) => c.id === id) as Chat;
      active = active.filter((c) => c.id !== id);
      archived = archived.filter((c) => c.id !== id);
      if (action === 'archive') archived = [{ ...chat, archived: '2026-11-09T12:00:00Z' }, ...archived];
      if (action === 'unarchive') active = [{ ...chat, archived: null }, ...active];
      return json({});
    }
    if (path === '/api/chats') return json(active);
    if (path === '/api/chats?archived=true') return json(archived);
    if (path === '/api/agents') return json({ agents: [] });
    if (path === '/api/projects') return json({ projects: [] });
    if (path === '/api/person-tasks') return json({ tasks: [], stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    if (action === 'tasks') return json({ tasks: [], waits: [] });
    if (id) return json({ chat: id, messages: [], turns: [], approvals: [], members: [] });
    return json({});
  });
}

const chat = (id: string): Chat => ({ id, title: `Made-up ${id}` });
const heading = (title: string) => page.getByRole('heading', { level: 1, name: title });

test('deleting the open chat asks first, then opens the next chat in the sidebar', async () => {
  service([chat('a'), chat('b'), chat('c')], []);
  await render(<App />);
  await page.getByRole('button', { name: 'Made-up b', exact: true }).click();
  await expect.element(heading('Made-up b')).toBeInTheDocument();

  await page.getByRole('main').getByRole('button', { name: 'More', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Delete' }).click();
  const confirm = page.getByRole('dialog', { name: 'Delete Made-up b?' });
  await expect.element(confirm.getByRole('button', { name: 'Cancel' })).toHaveFocus();
  await confirm.getByRole('button', { name: 'Delete' }).click();

  await expect.element(heading('Made-up c')).toBeInTheDocument();
  await expect.element(confirm).not.toBeInTheDocument();
  // Only the chat it opened: the deleted one's view is gone.
  expect(document.querySelectorAll('main').length).toBe(1);
  await expect.element(page.getByRole('button', { name: 'Made-up b', exact: true })).not.toBeInTheDocument();
});

test('the open chat deleted in another window gives way to its neighbour, as a Delete here does', async () => {
  let active = [chat('a'), chat('b'), chat('c')];
  service(active, []);
  await render(<App />);
  await page.getByRole('button', { name: 'Made-up b', exact: true }).click();
  await expect.element(heading('Made-up b')).toBeInTheDocument();

  active = active.filter((c) => c.id !== 'b');
  service(active, []);
  for (const on of live) on({ type: 'chat', chat: 'b', change: 'deleted' });
  await expect.element(heading('Made-up c')).toBeInTheDocument();
  await expect.element(page.getByRole('button', { name: 'Made-up b', exact: true })).not.toBeInTheDocument();
});

test('a refused Archive keeps the question open with its reference; Cancel gives focus back to the menu', async () => {
  service([chat('a')], [], (path) => (path.endsWith('/archive') ? refusal('Archive failed') : undefined));
  await render(<App />);
  await page.getByRole('button', { name: 'More for Made-up a' }).click();
  await page.getByRole('menuitem', { name: 'Archive' }).click();
  const confirm = page.getByRole('dialog', { name: 'Archive Made-up a?' });
  await confirm.getByRole('button', { name: 'Archive' }).click();

  await expect.element(confirm.getByText('Archive failed')).toBeVisible();
  await expect.element(confirm.getByRole('button', { name: 'Reference made-up-ref' })).toBeVisible();
  await confirm.getByRole('button', { name: 'Cancel' }).click();
  await expect.element(confirm).not.toBeInTheDocument();
  await expect.element(page.getByRole('button', { name: 'More for Made-up a' })).toHaveFocus();
  await expect.element(page.getByRole('button', { name: 'Made-up a', exact: true })).toBeVisible();
});

test("a More button's tooltip opening late never covers its menu", async () => {
  service([chat('a')], []);
  await render(<App />);
  const more = page.getByRole('button', { name: 'More for Made-up a' });
  await expect.element(more).toBeInTheDocument();
  const button = more.element();
  await more.click();
  // Radix opens the tooltip on a 0 ms timer that a press landing first doesn't cancel (WebKit, in CI).
  for (const type of ['pointerleave', 'pointermove'])
    button.dispatchEvent(new PointerEvent(type, { pointerType: 'mouse', bubbles: type === 'pointermove' }));
  await expect
    .poll(() => document.querySelector('[data-slot="tooltip-content"]')?.textContent)
    .toContain('More for Made-up a');

  await page.getByRole('menuitem', { name: 'Archive' }).click();
  await expect.element(page.getByRole('dialog', { name: 'Archive Made-up a?' })).toBeVisible();
});

test('an archived chat reads only until Restore; a refused Restore says why on it', async () => {
  let refuse = true;
  service([chat('a')], [{ ...chat('z'), archived: '2026-11-08T12:00:00Z' }], (path) =>
    path.endsWith('/unarchive') && refuse ? refusal('Unarchive failed') : undefined,
  );
  await render(<App />);
  const archive = page.getByRole('button', { name: /^Archived/ });
  await expect.element(archive).toHaveAttribute('aria-expanded', 'false');
  await archive.click();
  await page.getByRole('button', { name: 'More for Made-up z' }).click();
  await page.getByRole('menuitem', { name: 'Restore' }).click();

  // The failure opens the chat it's about, which still reads only.
  await expect.element(heading('Made-up z')).toBeInTheDocument();
  await expect.element(page.getByText('Unarchive failed')).toBeVisible();
  await expect.element(page.getByRole('textbox')).not.toBeInTheDocument();

  refuse = false;
  await page.getByRole('button', { name: 'Restore', exact: true }).click();
  await expect.element(page.getByRole('textbox')).toBeVisible();
  await expect.element(page.getByText('Unarchive failed')).not.toBeInTheDocument();
});
