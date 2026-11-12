// An agent's task opens in the Tasks tab with its PR's checks run, from Summary or the row of the turn
// that gave it (owner's pick A, room task 70; 0036 C6). The service is the edge: fetch answers as it would.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { App } from './App';
import type { CheckRun, Project, Task } from './api';
import type { LiveEvent } from './live';

vi.mock('./live', () => ({ listen: (_: (event: LiveEvent) => void) => () => {} }));
afterEach(() => vi.unstubAllGlobals());

const json = (body: unknown) => new Response(JSON.stringify(body));
const head = '8bd41c2e5f00a1b2c3d4e5f60718293a4b5c6d7e';
const members = [
  { agent: 'mira', name: 'Made-up Mira', role: 'supervisor' },
  { agent: 'sol', name: 'Made-up Sol', role: 'reviewer' },
];
const project: Project = {
  id: 'p',
  name: 'Made-up project',
  folder: 'C:/made-up',
  members: members.map((m) => ({ ...m, role: m.role as 'supervisor' | 'reviewer' })),
  checks: [],
  ci: 'gathr',
  merge: 'fast-forward',
};
const task: Task = {
  number: 82,
  chat: 'c',
  giver: 'mira',
  owner: 'sol',
  given_in: 'ask',
  kind: 'review',
  what: 'Verify the reconnect repair',
  state: 'working',
  reason: null,
  after: [],
  created_us: 0,
  pr: 42,
  pr_head: head,
};
const run: CheckRun = {
  run: 'r',
  head,
  state: 'passed',
  why: null,
  ref: null,
  started: '2026-11-11T10:40:00.000Z',
  ended: '2026-11-11T10:42:00.000Z',
  steps: [{ command: 'cargo test', exit: 0, duration_ms: 1000, log: 'ok' }],
};
const history = {
  chat: 'c',
  members,
  approvals: [],
  turns: [
    { id: 'ask', status: 'done', why: null, ref: 'a', agent: 'mira' },
    { id: 'rev', status: 'running', why: null, ref: 'b', agent: 'sol', task: 82 },
  ],
  messages: [
    { id: 'm1', turn: 'ask', time: '2026-11-11T10:00:00Z', role: 'user', kind: 'text', body: 'Review it' },
    {
      id: 'm2',
      turn: 'ask',
      time: '2026-11-11T10:00:30Z',
      role: 'assistant',
      kind: 'tool',
      body: { tool: 'mcp__gathr__give_task', status: 'ok' },
    },
    {
      id: 'm3',
      turn: 'ask',
      time: '2026-11-11T10:00:40Z',
      role: 'assistant',
      kind: 'text',
      body: 'Over to Sol',
    },
    { id: 'm4', turn: 'rev', time: '2026-11-11T10:01:00Z', role: 'gathr', kind: 'text', body: 'Task #82' },
  ],
};

function service() {
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/chats') return json([{ id: 'c', title: 'Made-up round', project: 'p' }]);
    if (path === '/api/chats?archived=true') return json([]);
    if (path === '/api/agents') return json({ agents: [] });
    if (path === '/api/projects') return json({ projects: [project] });
    if (path === '/api/person-tasks') return json({ tasks: [], stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    if (path === '/api/chats/c/tasks') return json({ tasks: [task], waits: [] });
    if (path === `/api/projects/p/checks/runs?head=${head}`) return json({ runs: [run] });
    if (path === '/api/chats/c') return json(history);
    return json({});
  });
}

test("a task in Summary opens in the Tasks tab with its PR's checks run; Back leaves it", async () => {
  service();
  await render(<App />);
  await page.getByRole('button', { name: 'Made-up round', exact: true }).click();
  await page.getByRole('button', { name: 'Summary' }).click();
  await page.getByRole('button', { name: /^Made-up Sol/ }).click();
  await page.getByRole('button', { name: 'Verify the reconnect repair' }).click();

  const opened = page.getByRole('region', { name: 'Task #82' });
  await expect.element(opened.getByRole('heading', { name: 'Checks passed' })).toBeVisible();
  await expect.element(opened.getByText('PR #42', { exact: false })).toBeVisible();
  await opened.getByRole('button', { name: 'Back' }).click();
  await expect.element(opened).not.toBeInTheDocument();
});

test('a task opens in the Tasks tab from the row of the turn that gave it', async () => {
  service();
  await render(<App />);
  await page.getByRole('button', { name: 'Made-up round', exact: true }).click();
  await page.getByRole('button', { name: /1 step · gave 1 task$/ }).click();
  await expect.element(page.getByText('Gave Made-up Sol a task')).toBeVisible();
  await page.getByRole('button', { name: '#82 Verify the reconnect repair' }).click();
  await expect.element(page.getByRole('region', { name: 'Task #82' })).toBeVisible();
});
