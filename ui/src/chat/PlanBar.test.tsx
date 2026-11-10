// The Agent plan bar above the composer: a long round still fits its row, and an opened plan stays open
// per chat, like the team column (Vale's #94 review).
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import '../theme.css';
import { App } from '../App';
import type { Chat } from '../api';
import type { LiveEvent } from '../live';

vi.mock('../live', () => ({ listen: (_: (event: LiveEvent) => void) => () => {} }));
afterEach(() => vi.unstubAllGlobals());

const json = (body: unknown) => new Response(JSON.stringify(body));
// The lead's hand-off gave twenty tasks: five done, the member at work on one.
const tasks = Array.from({ length: 20 }, (_, n) => ({
  number: n,
  chat: 'team',
  giver: 'a',
  owner: 'b',
  given_in: 'm',
  kind: 'work',
  what: `Made-up task ${n}`,
  state: n < 5 ? 'done' : n === 5 ? 'working' : 'ready',
  reason: null,
  after: [],
  created_us: 0,
}));
const team = {
  chat: 'team',
  messages: [
    { id: 'u', turn: 'm', time: '', role: 'user', kind: 'text', body: 'Made-up ask' },
    { id: 'l', turn: 'm', time: '', role: 'assistant', kind: 'text', body: 'Made-up hand-off' },
  ],
  turns: [
    { id: 'm', status: 'done', why: null, ref: 'm', agent: 'a', position: 0 },
    { id: 'w', status: 'running', why: null, ref: 'w', agent: 'b', task: 5, position: 1 },
  ],
  approvals: [],
  members: [
    { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
    { agent: 'b', name: 'Made-up Member', role: 'member' },
  ],
};
const chats: Chat[] = [
  { id: 'team', title: 'Made-up team' },
  { id: 'solo', title: 'Made-up solo' },
];

function service() {
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/chats') return json(chats);
    if (path === '/api/chats?archived=true') return json([]);
    if (path === '/api/agents') return json({ agents: [] });
    if (path === '/api/projects') return json({ projects: [] });
    if (path === '/api/person-tasks') return json({ tasks: [], stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    if (path === '/api/chats/team/tasks') return json({ tasks, waits: [] });
    if (path === '/api/chats/team') return json(team);
    if (path.endsWith('/tasks')) return json({ tasks: [], waits: [] });
    if (path === '/api/chats/solo')
      return json({ ...team, chat: 'solo', turns: [], messages: [], members: [] });
    return json({});
  });
}

const plan = () => page.getByRole('button', { name: /^Plan: / });

test('a twenty-step round in a narrow column keeps Plan, its line, done/total and the chevron in the row', async () => {
  service();
  await page.viewport(1024, 900);
  await render(<App />);
  await expect.element(plan()).toBeVisible();
  const row = plan().element() as HTMLElement;
  const bar = row.closest('.plan-bar') as HTMLElement;
  // Narrow enough that twenty full-size segments can't fit beside the rest (Vale measured 355px).
  expect(bar.clientWidth).toBeLessThan(420);
  const box = (selector: string) => (row.querySelector(selector) as HTMLElement).getBoundingClientRect();
  const edge = row.getBoundingClientRect().right;

  expect(row.scrollWidth).toBeLessThanOrEqual(row.clientWidth);
  const title = box('.plan-title');
  expect(title.width).toBeGreaterThanOrEqual(box('.plan-title strong').width);
  expect(box('.plan-title strong').width).toBeGreaterThan(0);
  expect(box('.plan-title span').width).toBeGreaterThan(20);
  expect(box('.plan-fraction').right).toBeLessThanOrEqual(edge);
  expect(box('.plan-chevron').right).toBeLessThanOrEqual(edge);
  expect(box('.plan-segments').right).toBeLessThanOrEqual(box('.plan-fraction').left);
  expect(row.querySelector('.plan-fraction')?.textContent).toBe('5/20');
});

test('the same round with room for it keeps every segment full size', async () => {
  service();
  await page.viewport(1440, 900);
  await render(<App />);
  await expect.element(plan()).toBeVisible();
  const segments = [...plan().element().querySelectorAll('.plan-segments i')];
  expect(segments.map((s) => s.getBoundingClientRect().width)).toEqual(Array(20).fill(12));
});

test('a plan opened in one chat is still open after switching to another chat and back', async () => {
  service();
  await page.viewport(1440, 900);
  await render(<App />);
  await plan().click();
  await expect.element(plan()).toHaveAttribute('aria-expanded', 'true');

  await page.getByRole('button', { name: 'Made-up solo', exact: true }).click();
  await expect.element(page.getByRole('heading', { level: 1, name: 'Made-up solo' })).toBeInTheDocument();
  await page.getByRole('button', { name: 'Made-up team', exact: true }).click();
  await expect.element(plan()).toHaveAttribute('aria-expanded', 'true');
  const steps = page.getByRole('listitem');
  await expect.element(steps.nth(19)).toBeInTheDocument();
  expect(steps.nth(4).element().textContent).toContain('Made-up task 4');
});
