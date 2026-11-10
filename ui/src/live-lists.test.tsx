// Chats and projects made outside this window (the round: a project and chat made through the API
// while the window was open). The service is the edge: fetch answers as it would.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { App } from './App';
import type { Chat, PersonTask, Project } from './api';
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
  projects: Project[];
  tasks: PersonTask[];
  refuse?: boolean;
  refuseAgents?: boolean;
  hold?: Promise<void>;
} = { chats: [], projects: [], tasks: [] };
function service() {
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/chats' && made.refuse)
      return json({ error: 'Chats failed', why: 'made-up refusal', ref: 'made-up-ref' }, 500);
    if (path === '/api/chats') {
      const chats = made.chats;
      await made.hold;
      return json(chats);
    }
    if (path === '/api/chats?archived=true') return json([]);
    if (path === '/api/agents' && made.refuseAgents)
      return json({ error: 'Agents failed', why: 'made-up agents refusal', ref: 'made-up-agents' }, 500);
    if (path === '/api/agents') return json({ agents: [] });
    if (path === '/api/projects') return json({ projects: made.projects });
    if (path === '/api/person-tasks') return json({ tasks: made.tasks, stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    const [, id, action] = path.match(/^\/api\/chats\/([^/?]+)\/?(\w*)$/) ?? [];
    if (action === 'tasks') return json({ tasks: [], waits: [] });
    if (id) return json({ chat: id, messages: [], turns: [], approvals: [], members: [] });
    return json({});
  });
}

const project: Project = { id: 'p1', name: 'Made-up project', folder: 'C:\\made-up', members: [] };
const chat: Chat = { id: 'c1', title: 'Made-up round', project: 'p1' };
const question: PersonTask = {
  number: 1,
  chat: 'c1',
  chat_title: 'Made-up round',
  giver: 'ada',
  owner: null,
  given_in: 't1',
  kind: 'person',
  what: 'Which label?',
  state: 'ready',
  reason: null,
  after: [],
  for_agent: null,
  why: null,
  form: 'text',
  choices: [],
  files: [],
  created_us: 0,
};

afterEach(() => {
  made.chats = [];
  made.projects = [];
  made.tasks = [];
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

test('a question whose chat the window has not loaded opens that chat at the question', async () => {
  made.tasks = [question];
  service();
  await render(<App />);
  // The service has the chat by now; this window never heard of it.
  made.projects = [project];
  made.chats = [chat];

  await page.getByRole('button', { name: /^Tasks/ }).click();
  await page.getByRole('button', { name: /^Which label\?/ }).click();
  await expect.element(page.getByRole('heading', { level: 1, name: 'Made-up round' })).toBeInTheDocument();
  await expect.element(page.getByRole('tab', { name: /^Tasks/ })).toHaveAttribute('aria-selected', 'true');
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

test('a question whose chat fails to load says why, with its reference', async () => {
  made.tasks = [question];
  service();
  await render(<App />);
  made.refuse = true;

  await page.getByRole('button', { name: /^Tasks/ }).click();
  await page.getByRole('button', { name: /^Which label\?/ }).click();
  await expect.element(page.getByText('Chats failed')).toBeVisible();
  await expect.element(page.getByRole('button', { name: 'Reference made-up-ref' })).toBeVisible();
});
