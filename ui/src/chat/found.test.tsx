// A search hit opened in its chat (plans/search.md): drawn and centred with only the pages it needs, and
// neither a stream nor a new turn pulls the view away from it.
import { afterEach, expect, test, vi } from 'vitest';
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

function Chat({ found, onFound }: { found: Found; onFound: () => void }) {
  return (
    <div style={{ height: 600, display: 'flex', flexDirection: 'column' }}>
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat' }}
        agents={[]}
        onEditAgent={() => {}}
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

test('a hit far up opens centred, and the view holds there while the chat goes on', async () => {
  // 299 messages from the end: the third page up.
  const { hit, viewport } = await openAt(long, 'a100');
  // The hit's own text, inside its reply.
  expect(hit.closest('[data-message-id]')?.getAttribute('data-message-id')).toBe('a-t100');
  expect(hit.textContent).toBe('Made-up answer 100');
  // Keys scroll the chat from it.
  expect(document.activeElement).toBe(hit);
  // Drawn from the hit's page down, not the whole chat.
  expect(viewport.querySelectorAll('[data-message-id]').length).toBeLessThan(2 * turns);

  // Where it shows, not scrollTop: an older page drawn above moves that and keeps the view.
  const held = hit.getBoundingClientRect().top;
  // As in WebKit, which has none: Chromium's scroll anchoring would hide a jump.
  viewport.style.overflowAnchor = 'none';
  // The agent's next turn starts, then streams a long reply.
  for (const listen of live)
    listen({ type: 'turn', chat: 'c', turn: 'next', status: 'running', agent: 'a', position: turns });
  await frames(3);
  for (const listen of live) listen({ type: 'delta', chat: 'c', turn: 'next', text: 'More. '.repeat(400) });
  await expect
    .poll(() => viewport.querySelector('[data-message-id="a-next"]')?.textContent?.length)
    .toBeGreaterThan(1000);
  await frames(3);
  expect(Math.abs(hit.getBoundingClientRect().top - held)).toBeLessThan(1);
});

test("a member's hit opens at its message in the one timeline", async () => {
  const { hit } = await openAt(team, 'f12');
  expect(hit.closest('[data-message-id]')?.getAttribute('data-message-id')).toBe('a-w12');
  expect(hit.textContent).toBe('Made-up finding 12');
});

test('a hit on what a turn wrote mid-work opens its row at those words', async () => {
  const { hit } = await openAt(team, 'r12');
  expect(hit.closest('[data-message-id]')?.getAttribute('data-message-id')).toBe('a-w12');
  expect(hit.closest('.work-steps')).not.toBeNull();
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

test('a reply that says the same thing twice opens at the passage the hit is', async () => {
  const same = 'Made-up identical passage.';
  const twice: History = {
    ...long,
    messages: [
      ...long.messages,
      text('u', 'tt', 'user', 'Made-up ask'),
      text('d1', 'tt', 'assistant', same),
      ...Array.from({ length: 40 }, (_, n) => text(`p${n}`, 'tt', 'assistant', `Made-up paragraph ${n}`)),
      text('d2', 'tt', 'assistant', same),
      ...Array.from({ length: 20 }, (_, n) => text(`q${n}`, 'tt', 'assistant', `Made-up close ${n}`)),
    ],
    turns: [...long.turns, done('tt', 'a', turns)],
  };
  const { hit } = await openAt(twice, 'd2');
  const passages = [...(hit.closest('[data-message-id]')?.querySelectorAll('.found-part') ?? [])];
  expect(passages).toHaveLength(2);
  expect(passages.indexOf(hit)).toBe(1);
});
