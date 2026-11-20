// Chats and projects made outside this window (the round: a project and chat made through the API
// while the window was open). The service is the edge: fetch answers as it would.
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

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });

const made: {
  chats: Chat[];
  archived: Chat[];
  projects: Project[];
  refuse?: boolean;
  refuseAgents?: boolean;
  hold?: Promise<void>;
} = { chats: [], archived: [], projects: [] };
function service() {
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/chats' && made.refuse)
      return json({ error: 'Chats failed', why: 'made-up refusal', ref: 'made-up-ref' }, 500);
    if (path === '/api/chats') {
      const chats = made.chats;
      await made.hold;
      return json(chats);
    }
    if (path === '/api/chats?archived=true') return json(made.archived);
    if (path === '/api/agents' && made.refuseAgents)
      return json({ error: 'Agents failed', why: 'made-up agents refusal', ref: 'made-up-agents' }, 500);
    if (path === '/api/agents') return json({ agents: [], removed: [] });
    if (path === '/api/projects') return json({ projects: made.projects });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    const [, id] = path.match(/^\/api\/chats\/([^/?]+)\/?(\w*)$/) ?? [];
    if (id) return json({ chat: id, messages: [], turns: [], approvals: [], members: [] });
    return json({});
  });
}

const project: Project = {
  id: 'p1',
  name: 'Made-up project',
  folder: 'C:\\made-up',
  members: [],
};
const chat: Chat = { id: 'c1', title: 'Made-up round', project: 'p1' };
afterEach(() => {
  made.chats = [];
  made.archived = [];
  made.projects = [];
  made.refuse = false;
  made.refuseAgents = false;
  made.hold = undefined;
});

test('a project and chat made elsewhere show in the sidebar, and the chat opens when none was', async () => {
  service();
  await render(<App />);
  await expect.element(page.getByText('No chats yet')).toBeVisible();

  made.projects = [project];
  made.chats = [chat];
  for (const on of live) on({ type: 'project', project: 'p1', change: 'created' });
  for (const on of live) on({ type: 'chat', chat: 'c1', change: 'created' });

  const sidebar = page.getByRole('navigation', { name: 'Chats' });
  await expect.element(sidebar.getByText('Made-up project')).toBeVisible();
  await expect.element(sidebar.getByRole('button', { name: 'Made-up round', exact: true })).toBeVisible();
  await expect.element(page.getByRole('heading', { level: 1, name: 'Made-up round' })).toBeInTheDocument();
});

test('a draft begun while the list reloads stays open when the reload lands', async () => {
  const open: Chat = { id: 'c0', title: 'Made-up open chat', project: null };
  made.chats = [open];
  service();
  await render(<App />);
  await expect
    .element(page.getByRole('heading', { level: 1, name: 'Made-up open chat' }))
    .toBeInTheDocument();

  let release = () => {};
  made.hold = new Promise((r) => {
    release = r;
  });
  made.chats = [{ id: 'c1', title: 'Made-up external chat', project: null }, open];
  for (const on of live) on({ type: 'chat', chat: 'c1', change: 'created' });
  await page.getByRole('button', { name: 'New', exact: true }).click();
  await page.getByRole('textbox', { name: 'Message' }).fill('Made-up draft');
  release();

  const sidebar = page.getByRole('navigation', { name: 'Chats' });
  await expect
    .element(sidebar.getByRole('button', { name: 'Made-up external chat', exact: true }))
    .toBeVisible();
  await expect.element(page.getByRole('heading', { level: 1, name: 'New chat' })).toBeInTheDocument();
  await expect.element(page.getByRole('textbox', { name: 'Message' })).toHaveValue('Made-up draft');
});

test('a failure while a chat is open shows above it, with why and its reference, until dismissed', async () => {
  made.chats = [{ id: 'c0', title: 'Made-up open chat', project: null }];
  service();
  await render(<App />);
  await expect
    .element(page.getByRole('heading', { level: 1, name: 'Made-up open chat' }))
    .toBeInTheDocument();

  made.refuseAgents = true;
  for (const on of live) on({ type: 'agents' } as LiveEvent);
  const notice = page.getByRole('alert').filter({ hasText: 'Agents failed' });
  await expect.element(notice.getByText('made-up agents refusal')).toBeVisible();
  await expect.element(notice.getByRole('button', { name: 'Reference made-up-agents' })).toBeVisible();
  await expect
    .element(page.getByRole('heading', { level: 1, name: 'Made-up open chat' }))
    .toBeInTheDocument();

  await notice.getByRole('button', { name: 'Dismiss' }).click();
  await expect.element(notice).not.toBeInTheDocument();
});

const other: Chat = { id: 'c1', title: 'Made-up other chat', project: null };
const openChat: Chat = { id: 'c0', title: 'Made-up open chat', project: null };
const tell = (event: LiveEvent) => {
  for (const on of live) on(event);
};
/** The sidebar row of the chat that isn't open, and its mark by name (0071). */
const otherRow = () =>
  page.getByRole('navigation', { name: 'Chats' }).getByRole('button', { name: /^Made-up other chat/ });
const dot = (name: string) => otherRow().getByRole('img', { name });

test('a chat that is not open shows Working while it works, Needs you once it needs the person', async () => {
  made.chats = [openChat, other];
  service();
  await render(<App />);
  await expect.element(otherRow()).toBeVisible();
  expect(document.querySelector('.chat-mark')).toBeNull();

  tell({ type: 'chat_state', chat: 'c1', working: true, needs: false, mark: 'working' });
  await expect.element(dot('Working')).toBeVisible();

  // The most pressing wins: one mark, and it says the person is needed.
  tell({ type: 'chat_state', chat: 'c1', working: true, needs: true, mark: 'needs_you' });
  await expect.element(dot('Needs you')).toBeVisible();
  expect(document.querySelectorAll('.chat-mark').length).toBe(1);

  tell({ type: 'chat_state', chat: 'c1', working: false, needs: false, mark: null });
  await expect.element(dot('Needs you')).not.toBeInTheDocument();
  expect(document.querySelector('.chat-mark')).toBeNull();
});

test('the mark sits 8px after the cut title, never on it, and leaves More where it is', async () => {
  made.chats = [
    openChat,
    { ...other, title: 'Made-up other chat with a title too long for its row in the sidebar' },
  ];
  service();
  await render(<App />);
  await expect.element(otherRow()).toBeVisible();
  const boxes = () => {
    const row = otherRow().element().parentElement as HTMLElement;
    const box = (e: Element | null) => (e as Element).getBoundingClientRect().toJSON();
    return [box(row.querySelector('.chat-link span')), box(row.querySelector('.row-more'))];
  };
  const before = boxes();
  tell({ type: 'chat_state', chat: 'c1', working: true, needs: false, mark: 'working' });
  await expect.element(dot('Working')).toBeVisible();
  const [title, more] = boxes();
  expect(more, 'More, before and after the mark').toEqual(before[1]);
  const at = dot('Working').element().getBoundingClientRect();
  expect(Math.round(at.left - title.right), 'the gap from the cut title to the mark').toBe(8);
  expect(at.right).toBeLessThanOrEqual(otherRow().element().getBoundingClientRect().right);
});

test('a reload after missed events shows each chat as the service has it', async () => {
  made.chats = [openChat, other];
  service();
  await render(<App />);
  await expect.element(otherRow()).toBeVisible();

  made.chats = [openChat, { ...other, working: true, needs: true, mark: 'needs_you' }];
  tell({ type: 'lagged' });
  await expect.element(dot('Needs you')).toBeVisible();
});

test('a state told while the list loads is kept when the older list lands', async () => {
  made.chats = [openChat, other];
  service();
  await render(<App />);
  await expect.element(otherRow()).toBeVisible();

  let release = () => {};
  made.hold = new Promise((r) => {
    release = r;
  });
  // The list was read before the turn started; its answer arrives after the turn's event. It names
  // the open chat anew: once that shows, the list has landed.
  made.chats = [{ ...openChat, title: 'Made-up renamed chat' }, other];
  tell({ type: 'lagged' });
  tell({ type: 'chat_state', chat: 'c1', working: true, needs: false, mark: 'working' });
  await expect.element(dot('Working')).toBeVisible();
  release();
  const sidebar = page.getByRole('navigation', { name: 'Chats' });
  await expect
    .element(sidebar.getByRole('button', { name: 'Made-up renamed chat', exact: true }))
    .toBeVisible();
  await expect.element(dot('Working')).toBeVisible();
});

test('a chat archived while it worked has no mark in Archived', async () => {
  made.chats = [openChat, { ...other, working: true, mark: 'working' }];
  service();
  await render(<App />);
  await expect.element(dot('Working')).toBeVisible();

  made.chats = [openChat];
  made.archived = [{ ...other, archived: '2026-11-11T00:00:00Z' }];
  tell({ type: 'chat', chat: 'c1', change: 'archived' });
  await page.getByRole('button', { name: /^Archived/ }).click();
  await expect.element(otherRow()).toBeVisible();
  expect(document.querySelector('.chat-mark')).toBeNull();
});
