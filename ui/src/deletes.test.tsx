// Deleting a project from its More (0069). The service is the edge: fetch
// answers as it would, and tells the live socket what it deleted.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { App } from './App';
import type { Chat, Project } from './api';
import type { LiveEvent } from './live';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('./live', () => ({
  listen: (on: (event: LiveEvent) => void) => {
    live.add(on);
    return () => live.delete(on);
  },
}));
afterEach(() => vi.unstubAllGlobals());

const json = (body: unknown) => new Response(JSON.stringify(body));
const tell = (event: LiveEvent) => {
  for (const on of live) on(event);
};
const project = (id: string, name: string): Project => ({
  id,
  name,
  folder: `X:\\made-up\\${id}`,
  members: [],
});
const chat = (id: string, project?: string): Chat => ({ id, title: `Made-up ${id}`, project });

interface Made {
  projects: Project[];
  chats: Chat[];
}

/** The service with these projects and chats; a project's delete deletes its chats, each told live. */
function service(made: Made) {
  const sent: string[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    const method = init?.method ?? 'GET';
    const [, id, more] = path.match(/^\/api\/projects\/(\w+)\/?(\w*)$/) ?? [];
    if (method === 'DELETE' && id) {
      sent.push(`${method} ${path}`);
      for (const c of made.chats.filter((c) => c.project === id)) {
        made.chats = made.chats.filter((x) => x !== c);
        tell({ type: 'chat', chat: c.id, change: 'deleted' });
      }
      made.projects = made.projects.filter((p) => p.id !== id);
      tell({ type: 'project', project: id, change: 'deleted' });
      return json({ chats: 2, tasks: 0, projects: 1, leads: [], deleted: true, left: [] });
    }
    if (more === 'delete') return json({ chats: 2, tasks: 0, projects: 1, leads: [] });
    if (path === '/api/chats') return json(made.chats);
    if (path === '/api/chats?archived=true') return json([]);
    if (path === '/api/agents') return json({ agents: [], removed: [] });
    if (path === '/api/projects') return json({ projects: made.projects });
    if (path === '/api/person-tasks') return json({ tasks: [], stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    const [, chatId, action] = path.match(/^\/api\/chats\/([^/?]+)\/?(\w*)$/) ?? [];
    if (action === 'tasks') return json({ tasks: [], waits: [] });
    if (chatId) return json({ chat: chatId, messages: [], turns: [], approvals: [], members: [] });
    return json({});
  });
  return sent;
}

const heading = (title: string) => page.getByRole('heading', { level: 1, name: title });

test('a project’s Delete says what goes, deletes it with its chats, and the open one gives way', async () => {
  const sent = service({
    projects: [project('p', 'Pantry'), project('m', 'Menus')],
    chats: [chat('a', 'p'), chat('b', 'p'), chat('w', 'm')],
  });
  await render(<App />);
  await page.getByRole('button', { name: 'Made-up a', exact: true }).click();
  await expect.element(heading('Made-up a')).toBeInTheDocument();

  await page.getByRole('button', { name: 'Pantry', exact: true }).hover();
  await page.getByRole('button', { name: 'More for Pantry' }).click();
  await page.getByRole('menuitem', { name: 'Delete' }).click();
  const confirm = page.getByRole('dialog', { name: 'Delete Pantry?' });
  await expect.element(confirm.getByText('2 chats will be deleted. Files are kept.')).toBeVisible();
  await confirm.getByRole('button', { name: 'Delete' }).click();

  await expect.element(confirm).not.toBeInTheDocument();
  await expect.element(page.getByRole('button', { name: 'More for Pantry' })).not.toBeInTheDocument();
  await expect.element(page.getByRole('button', { name: 'Made-up b', exact: true })).not.toBeInTheDocument();
  // Its whole group went, so the open chat gives way to New, never to a chat going with it.
  await expect.element(heading('New chat')).toBeInTheDocument();
  expect(sent).toEqual(['DELETE /api/projects/p']);
});
