// A search hit opened in its chat (plans/search.md): drawn and centred with only the pages it needs. Where the view
// goes as the chat grows after that is the chat library's (docs/testing.md), not tested.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { History } from '../api';
import type { LiveEvent } from '../live';
import { ChatView } from './ChatView';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('../live', () => ({
  listen: (listener: (event: LiveEvent) => void) => {
    live.add(listener);
    return () => live.delete(listener);
  },
}));
afterEach(() => {
  vi.unstubAllGlobals();
  live.clear();
});

const json = (body: unknown) => new Response(JSON.stringify(body));
const text = (id: string, turn: string, role: 'user' | 'assistant', body: string) =>
  ({ id, turn, time: '', role, kind: 'text', body }) as const;
const done = (id: string, agent: string, position: number, task?: number) =>
  ({ id, status: 'done', why: null, ref: id, agent, position, task }) as const;

// 250 asks and answers.
const turns = 250;
const long: History = {
  chat: 'c',
  messages: Array.from({ length: turns }, (_, n) => [
    text(`u${n}`, `t${n}`, 'user', `Made-up ask ${n}`),
    text(`a${n}`, `t${n}`, 'assistant', `Made-up answer ${n}`),
  ]).flat(),
  turns: Array.from({ length: turns }, (_, n) => done(`t${n}`, 'a', n)),
  approvals: [],
  members: [{ agent: 'a', name: 'Made-up Agent', role: 'supervisor' }],
};
// The lead hands out 30 tasks; each member reply is a tool call between two texts.
const team: History = {
  chat: 'c',
  messages: [
    text('u', 'm', 'user', 'Made-up ask'),
    text('l', 'm', 'assistant', 'Made-up hand-off'),
    ...Array.from({ length: 30 }, (_, n) => [
      text(`r${n}`, `w${n}`, 'assistant', `Made-up start ${n}`),
      {
        id: `x${n}`,
        turn: `w${n}`,
        time: '',
        role: 'assistant',
        kind: 'tool',
        body: { tool: 'Read', status: 'ok' },
      },
      text(`f${n}`, `w${n}`, 'assistant', `Made-up finding ${n}`),
    ]).flat(),
  ] as History['messages'],
  turns: [done('m', 'a', 0), ...Array.from({ length: 30 }, (_, n) => done(`w${n}`, 'b', n + 1, n))],
  approvals: [],
  members: [
    { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
    { agent: 'b', name: 'Made-up Member', role: 'member' },
  ],
};
const tasks = Array.from({ length: 30 }, (_, n) => ({
  number: n,
  chat: 'c',
  giver: 'a',
  owner: 'b',
  given_in: 'm',
  kind: 'work',
  what: `Task ${n}`,
  state: 'done',
  reason: null,
  after: [],
  created_us: 0,
}));

type Found = { message?: string; at: number };

function Chat({ found, onFound }: { found?: Found; onFound: () => void }) {
  return (
    <div style={{ height: 600, display: 'flex', flexDirection: 'column' }}>
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat' }}
        agents={[]}
        draft=""
        onDraft={() => {}}
        unsent={[]}
        onUnsent={() => {}}
        openSettings={() => {}}
        onNamed={() => {}}
        projects={[]}
        onProject={() => {}}
        onNewProject={() => {}}
        onStart={() => {}}
        pauses={{ pauses: [], usage: [] }}
        onPause={async () => {}}
        found={found}
        onFound={onFound}
      />
    </div>
  );
}

const frames = (n: number) =>
  new Promise<void>((resolve) => {
    const next = (left: number) => (left ? requestAnimationFrame(() => next(left - 1)) : resolve());
    next(n);
  });

/** Opens the chat at the hit; answers the tinted block and the chat's viewport once it's centred. */
async function openAt(history: History, found: string) {
  vi.stubGlobal('fetch', async (path: string) =>
    json(path.endsWith('/tasks') ? { tasks, waits: [] } : path.startsWith('/api/chats/') ? history : {}),
  );
  const onFound = vi.fn();
  const shown = await render(<Chat found={{ message: found, at: performance.now() }} onFound={onFound} />);
  await expect.poll(() => onFound.mock.calls.length).toBe(1);
  await frames(3);
  const hit = document.querySelector('[data-search-hit]') as HTMLElement;
  const viewport = hit.closest('.thread-viewport') as HTMLElement;
  const footer = viewport.querySelector('.thread-footer')?.getBoundingClientRect().height ?? 0;
  const view = viewport.getBoundingClientRect();
  const box = hit.getBoundingClientRect();
  expect(Math.abs(box.top + box.height / 2 - (view.top + (view.height - footer) / 2))).toBeLessThan(2);
  const reopen = (next: Found) => shown.rerender(<Chat found={next} onFound={onFound} />);
  return { hit, viewport, reopen, onFound };
}

test('a hit far up opens centred, with focus on it, drawn from its page down', async () => {
  // 299 messages from the end: the third page up.
  const { hit, viewport } = await openAt(long, 'a100');
  // The hit's own text, inside its reply.
  expect(hit.closest('[data-message-id]')?.getAttribute('data-message-id')).toBe('a-t100');
  expect(hit.textContent).toBe('Made-up answer 100');
  // Keys scroll the chat from it.
  expect(document.activeElement).toBe(hit);
  // Drawn from the hit's page down, not the whole chat.
  expect(viewport.querySelectorAll('[data-message-id]').length).toBeLessThan(2 * turns);
});

test("a member's hit opens at its message in the one timeline", async () => {
  const { hit } = await openAt(team, 'f12');
  expect(hit.closest('[data-message-id]')?.getAttribute('data-message-id')).toBe('a-w12');
  expect(hit.textContent).toBe('Made-up finding 12');
});

test('a hit on what a turn wrote mid-work opens at those words, in their place in its block', async () => {
  const { hit } = await openAt(team, 'r12');
  expect(hit.closest('[data-message-id]')?.getAttribute('data-message-id')).toBe('a-w12');
  expect(hit.textContent).toBe('Made-up start 12');
});
test("the chat's title, after a hit in it, opens at its latest message", async () => {
  const { viewport, reopen, onFound } = await openAt(long, 'a100');
  await reopen({ at: performance.now() });
  await expect.poll(() => onFound.mock.calls.length).toBeGreaterThan(1);
  await frames(3);
  expect(viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop).toBeLessThan(2);
  expect(document.querySelector('[data-search-hit]')).toBeNull();
});

test('a hit opened after Summary showed an approval stays the hit once it is done', async () => {
  // The long chat with a second agent, so it has Summary; its latest turn waiting on an approval.
  const asking = {
    ...long,
    members: [...long.members, { agent: 'b', name: 'Made-up Member', role: 'member' as const }],
    messages: [...long.messages, text('uq', 'tq', 'user', 'Made-up ask to run')],
    turns: [...long.turns, { ...done('tq', 'a', turns), status: 'running' }],
    approvals: [{ turn: 'tq', request_id: 'q1', tool: 'Bash', input: { command: 'made-up --cmd' } }],
  };
  vi.stubGlobal('fetch', async (path: string) =>
    json(path.endsWith('/tasks') ? { tasks: [], waits: [] } : path.startsWith('/api/chats/') ? asking : {}),
  );
  const onFound = vi.fn();
  const shown = await render(<Chat onFound={onFound} />);
  await shown.getByRole('button', { name: /^Summary: Made-up Agent needs you/ }).click();
  await page
    .getByRole('dialog', { name: 'Summary' })
    .getByRole('button', { name: /^Made-up Agent / })
    .click();
  await expect
    .poll(() => document.querySelector('[data-search-hit]')?.getAttribute('data-approval'))
    .toBe('q1');

  const calls = onFound.mock.calls.length;
  await shown.rerender(<Chat found={{ message: 'a100', at: performance.now() }} onFound={onFound} />);
  await expect.poll(() => onFound.mock.calls.length).toBeGreaterThan(calls);
  await frames(3);
  const hit = document.querySelector('[data-search-hit]') as HTMLElement;
  expect(hit.textContent).toBe('Made-up answer 100');
  // The app clears the hit once it is shown: the chat does not go back to the approval.
  await shown.rerender(<Chat onFound={onFound} />);
  await frames(3);
  expect(document.querySelector('[data-search-hit]')).toBe(hit);
});
