// The service is the edge here: fetch answers as it would, and live events arrive when the test says.
import { afterEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { WaitingLine as Line, Standing, Waiting } from '../api';
import type { LiveEvent } from '../live';
import { ChatView } from './ChatView';
import { headline } from './Waiting';

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

const minutesAgo = (n: number) => (Date.now() - n * 60_000) * 1000;
const line = (kind: string, what: string, moves: Line['moves'], more: Partial<Line> = {}): Line => ({
  kind,
  what,
  agent: null,
  task: null,
  since_us: minutesAgo(9),
  moves,
  seen: null,
  looked_us: null,
  error: null,
  silent_us: null,
  ...more,
});
/** The lines a test shows, made when it runs: their ages are counted from then. */
function made() {
  const ci = line('wait', 'Made-up Member waits for CI on #157', 'itself', {
    seen: '1 of 2 checks on #157 at 0123456 finished: 1 passed; queued: UI (macOS)',
    looked_us: minutesAgo(1),
  });
  const blocked = line(
    'trouble',
    '#4 (Made-up Third) is blocked: left: one file. Made-up Lead was told',
    'nobody',
  );
  const question = line('question', 'A question for you: Pick a colour', 'you');
  const failing = line('wait', 'Made-up Lead waits for a check', 'nobody', { error: 'gh: not signed in' });
  // A running turn with no progress: it began 30 minutes ago and last did something 12 minutes ago.
  const silent = line('turn', 'Made-up Member is working on #4', 'nobody', {
    agent: 'b',
    task: 4,
    since_us: minutesAgo(30),
    silent_us: minutesAgo(12),
  });
  return { ci, blocked, question, failing, silent };
}

const members = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
  { agent: 'b', name: 'Made-up Member', role: 'member' },
];
const history = { chat: 'c', messages: [], turns: [], approvals: [], members };
const agents: Standing[] = [
  { agent: 'b', name: 'Made-up Member', state: 'working', words: 'Working', on: [], until_us: null },
];

// What the service answers now; a test changes it as the service would, then says so live.
let answer: Waiting = { lines: [], agents: [] };
let asked = 0;

function service(waiting: Waiting, turns: object[] = []) {
  answer = waiting;
  asked = 0;
  vi.stubGlobal('fetch', async (path: string) => {
    if (path.endsWith('/waiting')) {
      asked += 1;
      return new Response(JSON.stringify(answer));
    }
    if (path.endsWith('/tasks')) return new Response(JSON.stringify({ tasks: [], waits: [] }));
    return new Response(JSON.stringify({ ...history, turns }));
  });
}

const Chat = () => (
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
  />
);

const foot = () => page.getByRole('button', { name: /^(Waiting|Not moving)/ });
const list = page.getByRole('dialog', { name: 'What this chat waits on' });
/** The open list: each group's heading with its count, and what its lines say. */
const groups = () =>
  [...document.querySelectorAll('.waiting-list section')].map((s) => [
    s.querySelector('h3')?.textContent,
    [...s.querySelectorAll('li')].map((li) => li.querySelector('.waiting-what')?.textContent),
  ]);
const read = () => document.querySelector('.waiting-list')?.textContent ?? '';

test('the line says what waits on the person first, then what has waited longest, then what nobody is on', () => {
  const { ci, blocked, question, silent } = made();
  expect(headline([ci, blocked], agents)).toMatchObject({
    lead: 'Waiting',
    words: 'Made-up Member waits for CI on #157',
    more: 1,
  });
  expect(headline([blocked], agents)).toMatchObject({ lead: 'Not moving', words: blocked.what, more: 0 });
  // Lines that only follow others aren't stuck.
  const follows = line('trouble', '#4 is blocked. Made-up Lead hears of it next', 'others');
  expect(headline([follows], agents)).toMatchObject({ lead: 'Waiting', moves: 'others', more: 0 });
  const approval = line('approval', 'Made-up Member needs your approval', 'you');
  const paused = line('queued', "Made-up Lead's turn is queued: paused", 'you');
  expect(headline([question, { ...question, what: 'Another' }, approval, paused, ci], agents)).toMatchObject({
    lead: 'Waiting on you',
    words: '2 questions, 1 approval, 1 paused',
    more: 1,
  });
  // A silent turn comes after what waits on the person and before the rest, by its agent's name.
  expect(headline([blocked, silent, ci], agents)).toMatchObject({
    moves: 'nobody',
    lead: 'Working',
    words: 'Made-up Member silent',
    silent: silent.silent_us,
    more: 2,
  });
  expect(headline([question, silent], agents)).toMatchObject({ lead: 'Waiting on you', more: 1 });
  // A turn sent to no agent of the chat.
  expect(headline([{ ...silent, agent: null }], agents)).toMatchObject({ words: 'An agent silent' });
});

test('with no turn running, the foot of the chat says what it waits on and for how long, and opens the whole list', async () => {
  const { ci, blocked, failing } = made();
  service({ lines: [blocked, failing, ci], agents: [] });
  const screen = await render(<Chat />);
  // Scenario 6b: the queued macOS job nobody saw. The line names the wait, and the list what its look saw.
  await expect
    .element(foot())
    .toHaveTextContent('Waiting: Made-up Member waits for CI on #157· 9mand 2 more');
  await expect
    .element(screen.getByRole('status').filter({ hasText: 'Waiting: Made-up Member waits for CI on #157' }))
    .toBeInTheDocument();
  await foot().click();
  await expect.element(list).toBeVisible();
  expect(groups()).toEqual([
    ['Not moving2', [blocked.what, failing.what]],
    ['Moving by itself1', [ci.what]],
  ]);
  expect(read()).toContain('Last look 1m ago: 1 of 2 checks on #157');
  expect(read()).toContain('Its looks fail: gh: not signed in');
  await userEvent.keyboard('{Escape}');
  await expect.element(list).not.toBeInTheDocument();
});

test('the line follows the service without a reload, and goes when the last hold ends', async () => {
  const { ci, question } = made();
  service({ lines: [ci], agents: [] });
  await render(<Chat />);
  await expect.element(foot()).toHaveTextContent('Waiting: Made-up Member waits for CI on #157· 9m');
  // A question is asked: the service says the chat's answer may have changed, and the window asks again.
  answer = { lines: [question, ci], agents: [] };
  for (const listener of live) listener({ type: 'waiting', chat: 'c' });
  await expect.element(foot()).toHaveTextContent('Waiting on you: 1 questionand 1 more');
  // Another chat's change asks nothing.
  const before = asked;
  for (const listener of live) listener({ type: 'waiting', chat: 'other' });
  await new Promise((r) => setTimeout(r, 50));
  expect(asked).toBe(before);
  // The last hold ends.
  answer = { lines: [], agents: [] };
  for (const listener of live) listener({ type: 'waiting', chat: 'c' });
  await expect.element(foot()).not.toBeInTheDocument();
});

test('a turn that is queued and has not started is said, not hidden as if it ran', async () => {
  const queued = line(
    'queued',
    "Made-up Member's turn is queued: behind this agent's turn in another chat",
    'itself',
  );
  service({ lines: [queued], agents: [] }, [{ id: 't', status: 'queued', why: null, ref: 'r', agent: 'b' }]);
  await render(<Chat />);
  await expect
    .element(foot())
    .toHaveTextContent(
      "Waiting: Made-up Member's turn is queued: behind this agent's turn in another chat· 9m",
    );
});

test('while a turn runs the foot says Working, not what waits', async () => {
  const { ci } = made();
  service({ lines: [ci], agents: [] }, [{ id: 't', status: 'running', why: null, ref: 'r', agent: 'b' }]);
  const screen = await render(<Chat />);
  await expect.element(screen.getByRole('button', { name: /Working/ })).toBeVisible();
  await expect.element(foot()).not.toBeInTheDocument();
});

test('a silent turn says who and for how long, and opens the list', async () => {
  const { silent } = made();
  service({ lines: [silent], agents }, [{ id: 't', status: 'running', why: null, ref: 'r', agent: 'b' }]);
  const screen = await render(<Chat />);
  // Scenario 6d: in Working's place, so the line opens the list.
  const quiet = screen.getByRole('button', { name: /^Working: / });
  await expect.element(quiet).toHaveTextContent('Working: Made-up Member silent 12m');
  expect(document.querySelector('.working-spinner')).toBeNull();
  await quiet.click();
  await expect.element(list).toBeVisible();
  expect(groups()).toEqual([['Not moving1', [silent.what]]]);
  expect(read()).toContain('silent 12m');
  // The turn makes progress again: the service says the answer may have changed, and Working is back.
  answer = { lines: [{ ...silent, moves: 'itself', silent_us: null }], agents };
  for (const listener of live) listener({ type: 'waiting', chat: 'c' });
  await expect.element(quiet).not.toBeInTheDocument();
  await expect.element(screen.getByRole('button', { name: /^Working Made-up Member/ })).toBeVisible();
});
