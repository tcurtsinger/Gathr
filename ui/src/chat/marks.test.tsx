// Every agent's state as a mark on its face (0071): the top line's roster, a message's face, the card, an archived
// chat and an agent that left. The service is the edge: fetch answers as it would. How a mark is drawn is not
// tested here: the pictures prove that.
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { cdp, page, server, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Chat, Member, Message, Standing, Task, Turn } from '../api';
import type { LiveEvent } from '../live';
import { ChatView } from './ChatView';
import '../app.css';

vi.mock('../live', () => ({ listen: (_: (event: LiveEvent) => void) => () => {} }));
beforeEach(() => page.viewport(1280, 800));
afterEach(() => vi.unstubAllGlobals());

const members: Member[] = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
  { agent: 'b', name: 'Made-up Member', role: 'member' },
  { agent: 'c', name: 'Made-up Third', role: 'member' },
];
const agent = (id: string, name: string): Agent => ({
  id,
  name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto',
  face_seed: 'Made-up',
  gender: null,
  skin: null,
  background: null,
});
const agents = members.map((m) => agent(m.agent, m.name));
const turn = (id: string, agentId: string, position: number, more: Partial<Turn> = {}): Turn => ({
  id,
  agent: agentId,
  status: 'done',
  why: null,
  ref: id,
  position,
  ...more,
});
const said = (id: string, turnId: string, body: string): Message => ({
  id,
  turn: turnId,
  time: '2026-11-08T09:00:00Z',
  role: 'assistant',
  kind: 'text',
  body,
});
// The lead answered; the member wrote, then the third's turn failed, then the member works again. An agent that
// left the chat (z) wrote before it went.
const history = {
  chat: 'c',
  members,
  approvals: [],
  turns: [
    turn('ask', 'a', 0),
    turn('one', 'b', 1),
    turn('gone', 'z', 2),
    turn('fail', 'c', 3, { status: 'failed', task: 8 }),
    turn('now', 'b', 4, { status: 'running', created: new Date().toISOString() }),
  ],
  messages: [
    said('m1', 'ask', 'Made-up plan'),
    said('m2', 'one', 'Made-up finding'),
    said('m3', 'gone', 'Made-up words from an agent that left'),
    said('m4', 'fail', 'Made-up half a reply'),
    said('m5', 'now', 'Made-up progress'),
  ],
};
const task = (number: number, owner: string): Task => ({
  number,
  chat: 'c',
  giver: 'a',
  owner,
  given_in: 'ask',
  kind: 'work',
  what: `Made-up task ${number}`,
  state: 'failed',
  reason: null,
  after: [],
  created_us: 0,
});
const stands = (id: string, state: Standing['state'], more: Partial<Standing> = {}): Standing => ({
  agent: id,
  name: 'Made-up',
  state,
  words: 'Made-up',
  what: null,
  task: null,
  since_us: null,
  on: [],
  until_us: null,
  failing: false,
  ...more,
});
const twoMinutes = () => (Date.now() - 125_000) * 1000;

function service(
  third = stands('c', 'cant_run', { what: 'Failed', task: 8, since_us: twoMinutes() }),
  said = history,
) {
  const waiting = { lines: [], agents: [stands('a', 'free'), stands('b', 'working'), third], gathr: [] };
  vi.stubGlobal('fetch', async (path: string) => {
    if (path.endsWith('/waiting')) return new Response(JSON.stringify(waiting));
    if (path.endsWith('/tasks')) return new Response(JSON.stringify({ tasks: [task(8, 'c')], waits: [] }));
    return new Response(JSON.stringify(path === '/api/pauses' ? {} : said));
  });
}

function chat(more: Partial<Chat> = {}) {
  return render(
    <div className="app">
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat', project: 'p', ...more }}
        agents={agents}
        removed={[
          { id: 'z', name: 'Made-up Gone', face_seed: 'Made-up', gender: null, skin: null, background: null },
        ]}
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
      />
    </div>,
  );
}

/** Each face's agent, its mark and whether that mark turns: "b:working turning". */
const faces = (within: string) =>
  [...document.querySelectorAll(`${within} .face`)].map((face) => {
    const mark = face.querySelector('.mark');
    const turns = mark?.hasAttribute('data-spin') ? ' turning' : '';
    return `${face.getAttribute('data-agent')}:${mark?.getAttribute('data-state') ?? '-'}${turns}`;
  });

test('the top line shows every agent of the chat in its order, each with its mark; every Working mark turns', async () => {
  service();
  await chat();
  await expect.poll(() => faces('.topline .roster')).toEqual(['a:free', 'b:working turning', 'c:cant']);
  // A message's face carries the same mark, turning on every one of a member at work (the owner's call of
  // 2026-11-19: a ring standing still on an older message read as stuck). One that left has none.
  expect(faces('.thread-messages')).toEqual([
    'a:free',
    'b:working turning',
    'z:-',
    'c:cant',
    'b:working turning',
  ]);
  await expect.element(page.getByRole('button', { name: /^Summary/ })).toBeVisible();
});

test('an agent at work before its first word has its bubble, and its Working mark turns on that face', async () => {
  service(undefined, { ...history, messages: history.messages.filter((m) => m.turn !== 'now') });
  await chat();
  await expect
    .poll(() => faces('.thread-messages'))
    .toEqual(['a:free', 'b:working turning', 'z:-', 'c:cant', 'b:working turning']);
});

test('a face’s card names the state, what and since when, by pointer and by keyboard focus', async () => {
  service();
  const screen = await chat();
  const third = screen.getByRole('button', { name: 'Open Made-up Third' });
  await expect.poll(() => faces('.topline .roster')).toContain('c:cant');
  const card = page.getByText('Can’t run · Failed · Made-up task 8 · 2m');

  await userEvent.hover(third);
  await expect.element(card).toBeVisible();
  await expect.element(page.getByText('Made-up Third', { exact: true }).last()).toBeVisible();
  await userEvent.unhover(third);
  await expect.element(card).not.toBeInTheDocument();

  // Focus set by hand: WebKit's Tab passes over buttons.
  (third.element() as HTMLElement).focus();
  await expect.element(card).toBeVisible();
  (third.element() as HTMLElement).blur();
  await expect.element(card).not.toBeInTheDocument();
});

test('a face’s card says Can’t read, as the status list’s row does, while the looks at its wait keep failing', async () => {
  const hour = (Date.now() - 64 * 60_000) * 1000;
  service(
    stands('c', 'waiting', { on: [{ what: 'CI on #18', since_us: hour }], since_us: hour, failing: true }),
  );
  const screen = await chat();
  await expect.poll(() => faces('.topline .roster')).toContain('c:waiting');
  // Focus set by hand: WebKit's Tab passes over buttons.
  (screen.getByRole('button', { name: 'Open Made-up Third' }).element() as HTMLElement).focus();
  await expect.element(page.getByText('Waiting · PR #18 checks · Can’t read · 1h 4m')).toBeVisible();
});
test('an agent that left the chat has no mark and is off the top line; its card says Removed', async () => {
  service();
  await chat();
  await expect.poll(() => faces('.topline .roster')).toEqual(['a:free', 'b:working turning', 'c:cant']);
  const gone = document.querySelector('.thread-messages .face[data-agent="z"]') as HTMLElement;
  await userEvent.hover(gone);
  await expect.element(page.getByText('Removed', { exact: true })).toBeVisible();
});

test('an archived chat shows no marks anywhere', async () => {
  service();
  await chat({ archived: '2026-11-17T10:00:00Z' });
  await expect.poll(() => faces('.topline .roster')).toEqual(['a:-', 'b:-', 'c:-']);
  await expect.poll(() => faces('.thread-messages').length).toBe(5);
  expect(document.querySelectorAll('.mark')).toHaveLength(0);
});

// Reduced motion is the browser's setting, and the tests run with it on: only Chromium lets a test switch it.
test.runIf(server.browser === 'chromium')(
  'the Working mark turns, and stands still under reduced motion',
  async () => {
    service();
    await chat();
    await expect.poll(() => faces('.topline .roster')).toContain('b:working turning');
    const turning = () =>
      getComputedStyle(document.querySelector('.roster .mark[data-spin] > svg') as Element);
    const motion = (value: string) =>
      cdp().send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value }] });
    await motion('no-preference');
    try {
      expect(turning().animationName).toBe('mark-turn');
      await motion('reduce');
      expect(turning().animationName).toBe('none');
    } finally {
      // Back to what the tests run with: clearing the override would leave the system's own setting.
      await motion('reduce');
    }
  },
);
