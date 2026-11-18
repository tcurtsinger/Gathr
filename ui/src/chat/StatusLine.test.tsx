// The status line above the message box: how the team stands, from the service's answer and the chat as it
// streams (0050, 0071), and the list it opens. The service is the edge: fetch answers as it would, and live events
// arrive when the test says. How the line is drawn is not tested here: the pictures prove that.
import { useState } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { cdp, page, server, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Member, Message, Own, Standing, Task, Turn, Waiting, WaitingLine } from '../api';
import type { LiveEvent } from '../live';
import { ChatView } from './ChatView';
import '../app.css';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('../live', () => ({
  listen: (listener: (event: LiveEvent) => void) => {
    live.add(listener);
    return () => live.delete(listener);
  },
}));
beforeEach(() => page.viewport(1280, 800));
afterEach(() => {
  vi.unstubAllGlobals();
  live.clear();
});

const members: Member[] = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
  { agent: 'b', name: 'Made-up Member', role: 'member' },
  { agent: 'c', name: 'Made-up Third', role: 'reviewer' },
];
const agents: Agent[] = members.map((m) => ({
  id: m.agent,
  name: m.name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto',
  face_seed: 'Made-up',
  gender: null,
  skin: null,
  background: null,
}));
const ago = (ms: number) => Date.now() - ms;
const turn = (id: string, agent: string, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  ...more,
});
const said = (
  id: string,
  turnId: string | null,
  body: string,
  role: Message['role'] = 'assistant',
): Message => ({
  id,
  turn: turnId,
  time: '2026-11-08T09:00:00Z',
  role,
  kind: 'text',
  body,
});
const task = (number: number, owner: string, more: Partial<Task> = {}): Task => ({
  number,
  chat: 'c',
  giver: 'a',
  owner,
  given_in: 'ask',
  kind: 'work',
  what: `Made-up task ${number}`,
  state: 'working',
  reason: null,
  after: [],
  created_us: 0,
  ...more,
});
const stands = (agent: string, state: Standing['state'], more: Partial<Standing> = {}): Standing => ({
  agent,
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
const free = (agent: string) => stands(agent, 'free', { words: 'Free' });

// What the service answers now; a test changes it as the service would, then says so live. Gathr has nothing of
// its own unless a test says so.
type Said = Omit<Waiting, 'gathr'> & Partial<Waiting>;
let answer: Waiting = { lines: [], agents: [], gathr: [] };
function service(waiting: Said, more: { turns?: Turn[]; tasks?: Task[]; messages?: Message[] } = {}) {
  answer = { gathr: [], ...waiting };
  const history = {
    chat: 'c',
    members,
    approvals: [],
    turns: [turn('ask', 'a'), ...(more.turns ?? [])],
    messages: [said('m1', 'ask', 'Made-up plan'), ...(more.messages ?? [])],
  };
  vi.stubGlobal('fetch', async (path: string) => {
    if (path.endsWith('/waiting')) return new Response(JSON.stringify(answer));
    if (path.endsWith('/tasks')) return new Response(JSON.stringify({ tasks: more.tasks ?? [], waits: [] }));
    return new Response(JSON.stringify(path === '/api/pauses' ? {} : history));
  });
}
const changed = (waiting: Said) => {
  answer = { gathr: [], ...waiting };
  for (const listener of live) listener({ type: 'waiting', chat: 'c' });
};

/** The chat with a stand-in for the app's workspace, showing the open agent's name. */
function Chat() {
  const [agent, setAgent] = useState<string>();
  return (
    <div className="app" style={{ height: 760 }}>
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat' }}
        agents={agents}
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
        agentOpen={agent === undefined ? undefined : { agent, shown: true }}
        onOpenAgent={setAgent}
        workspace={(open) => (
          <aside aria-label="Made-up workspace" className="workspace" style={{ width: 440 }}>
            {open && <h2>{open.name}</h2>}
          </aside>
        )}
      />
    </div>
  );
}

const slot = () => document.querySelector('.status-slot') as HTMLElement;
const words = () => document.querySelector('.status-line')?.textContent ?? '';
const box = () => document.querySelector('.composer') as HTMLElement;
const list = page.getByRole('dialog', { name: 'Team' });
/** Each row of the open list: the agent's name, then its state, what and how long. */
const rows = () =>
  [...document.querySelectorAll('.status-row')].map((r) => [
    r.querySelector('.status-row-name')?.textContent,
    r.querySelector('.status-row-state')?.textContent,
  ]);

test('the line says how the team stands, follows the service without a reload, and keeps its slot when it empties', async () => {
  // The lead asked the person a question; the reviewer waits on a PR's checks, in the service's words.
  const since = ago(5 * 60_000) * 1000;
  service({
    lines: [],
    agents: [
      stands('a', 'needs_you', { what: 'Question', since_us: since }),
      free('b'),
      stands('c', 'waiting', { on: [{ what: 'CI on #18', since_us: since }] }),
    ],
  });
  await render(<Chat />);
  await expect.poll(words).toBe('Made-up Lead needs you, Made-up Third is waiting');
  expect(slot().querySelector('[data-tone="needs"]')?.textContent).toBe('Made-up Lead needs you');
  // A pull request's checks in the approved words, when the line waits alone on one thing.
  changed({ lines: [], agents: [free('a'), free('b'), answer.agents[2]] });
  await expect.poll(words).toBe('Made-up Third is waiting on PR #18 checks');
  const before = { slot: slot().getBoundingClientRect().height, box: box().getBoundingClientRect().top };
  // Everyone free: no line, and nothing above or below it moves.
  changed({ lines: [], agents: [free('a'), free('b'), free('c')] });
  await expect.poll(words).toBe('');
  expect(slot().getBoundingClientRect().height).toBe(28);
  expect({ slot: slot().getBoundingClientRect().height, box: box().getBoundingClientRect().top }).toEqual(
    before,
  );
});

test('the dots end the line only while an agent works, and stand still under reduced motion', async () => {
  // The member's turn runs; the reviewer has a task under way and no turn.
  service(
    { lines: [], agents: [free('a'), stands('b', 'working'), stands('c', 'waiting')] },
    {
      turns: [turn('now', 'b', { status: 'running', created: new Date().toISOString() })],
      tasks: [task(4, 'c')],
    },
  );
  const screen = await render(<Chat />);
  await expect.poll(words).toBe('Made-up Member is working, Made-up Third is waiting');
  expect(document.querySelectorAll('.status-dots > i')).toHaveLength(3);
  // Nobody works: the line stays, without its dots.
  service({ lines: [], agents: [free('a'), free('b'), stands('c', 'waiting')] }, { tasks: [task(4, 'c')] });
  await screen.rerender(<Chat key="again" />);
  await expect.poll(words).toBe('Made-up Third is waiting');
  expect(document.querySelector('.status-dots')).toBeNull();
});

// Reduced motion is the browser's setting, and the tests run with it on: only Chromium lets a test switch it.
test.runIf(server.browser === 'chromium')(
  'a light passes along the dots, and they are still under reduced motion',
  async () => {
    service(
      { lines: [], agents: [free('a'), stands('b', 'working'), free('c')] },
      { turns: [turn('now', 'b', { status: 'running', created: new Date().toISOString() })] },
    );
    await render(<Chat />);
    await expect.poll(() => document.querySelectorAll('.status-dots > i').length).toBe(3);
    const dot = () => getComputedStyle(document.querySelector('.status-dots > i') as Element);
    const motion = (value: string) =>
      cdp().send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value }] });
    await motion('no-preference');
    try {
      expect(dot().animationName).toBe('status-pass');
      await motion('reduce');
      expect(dot().animationName).toBe('none');
    } finally {
      await motion('reduce');
    }
  },
);

test('the line opens a row for each agent that isn’t free, with its state, what and how long; a row opens its tab', async () => {
  const twoMinutes = ago(125_000) * 1000;
  service(
    {
      lines: [],
      agents: [
        stands('a', 'needs_you', { what: 'Question', since_us: twoMinutes }),
        stands('b', 'working'),
        stands('c', 'blocked', { task: 4, since_us: twoMinutes }),
      ],
    },
    {
      // The member's turn began 40 seconds ago, on task 7.
      turns: [turn('now', 'b', { status: 'running', task: 7, created: new Date(ago(40_000)).toISOString() })],
      tasks: [task(7, 'b'), task(4, 'c', { state: 'blocked' })],
    },
  );
  await render(<Chat />);
  const line = page.getByRole('button', {
    name: 'Made-up Lead needs you, Made-up Member is working, Made-up Third is blocked',
  });
  await expect.element(line).toHaveAttribute('aria-expanded', 'false');
  await line.click();
  await expect.element(list).toBeVisible();
  // Seconds under a minute, as the face's card says them.
  expect(rows()).toEqual([
    ['Made-up Lead', 'Needs you · Question · 2m'],
    ['Made-up Member', expect.stringMatching(/^Working · Made-up task 7 · 4\ds$/)],
    ['Made-up Third', 'Blocked · Made-up task 4 · 2m'],
  ]);
  await userEvent.keyboard('{Escape}');
  await expect.element(list).not.toBeInTheDocument();
  await expect.element(line).toHaveFocus();

  await line.click();
  await list.getByRole('button', { name: /^Made-up Third/ }).click();
  await expect.element(list).not.toBeInTheDocument();
  await expect
    .element(page.getByRole('complementary', { name: 'Made-up workspace' }).getByRole('heading'))
    .toHaveTextContent('Made-up Third');
});

test('a quiet chat still tells the person: a silent turn says who and for how long, and Gathr says what isn’t moving', async () => {
  // The member's turn began 30 minutes ago and has made no progress for 12; the reviewer's task is blocked.
  const silent: WaitingLine = {
    kind: 'turn',
    what: 'Made-up Member is working on #7',
    agent: 'b',
    task: 7,
    since_us: ago(30 * 60_000) * 1000,
    moves: 'nobody',
    seen: null,
    looked_us: null,
    error: null,
    silent_us: ago(12 * 60_000) * 1000,
  };
  service(
    { lines: [silent], agents: [free('a'), stands('b', 'working'), stands('c', 'blocked', { task: 4 })] },
    {
      turns: [
        turn('now', 'b', { status: 'running', task: 7, created: new Date(ago(30 * 60_000)).toISOString() }),
      ],
      tasks: [task(7, 'b'), task(4, 'c', { state: 'blocked' })],
      // What Gathr tells the person once the chat has been stuck 5 minutes (0052).
      messages: [
        said('g1', null, 'Not moving for 6 min: #4 (Made-up Third). Write here to move it on.', 'gathr'),
      ],
    },
  );
  await render(<Chat />);
  await expect
    .element(page.getByText('Not moving for 6 min: #4 (Made-up Third)', { exact: false }))
    .toBeVisible();
  await expect.poll(words).toBe('Made-up Member is working, Made-up Third is blocked');
  await page.getByRole('button', { name: /^Made-up Member is working/ }).click();
  // The free lead has no row.
  expect(rows()).toEqual([
    ['Made-up Member', 'Working · Made-up task 7 · silent 12m'],
    ['Made-up Third', 'Blocked · Made-up task 4'],
  ]);
});

test('a wait whose looks keep failing says Can’t read on its row, until a look works', async () => {
  // The reviewer waits on PR #18's checks and every look at them fails; the member waits on PR #19's, looks fine.
  const hour = ago(64 * 60_000) * 1000;
  const five = ago(5 * 60_000) * 1000;
  const third = (failing: boolean) =>
    stands('c', 'waiting', { on: [{ what: 'CI on #18', since_us: hour }], since_us: hour, failing });
  const member = stands('b', 'waiting', { on: [{ what: 'CI on #19', since_us: five }], since_us: five });
  service({ lines: [], agents: [free('a'), member, third(true)] });
  await render(<Chat />);
  // The line is as it was: the reviewer is still waiting.
  await expect.poll(words).toBe('Made-up Member and Made-up Third are waiting');
  await page.getByRole('button', { name: 'Made-up Member and Made-up Third are waiting' }).click();
  await expect.element(list).toBeVisible();
  expect(rows()).toEqual([
    ['Made-up Member', 'Waiting · PR #19 checks · 5m'],
    ['Made-up Third', 'Waiting · PR #18 checks · Can’t read · 1h 4m'],
  ]);
  expect(
    [...document.querySelectorAll('.status-cant')].map((c) => c.closest('.status-row')?.textContent),
  ).toEqual(['Made-up ThirdWaiting · PR #18 checks · Can’t read · 1h 4m']);

  changed({ lines: [], agents: [free('a'), member, third(false)] });
  await expect.element(page.getByText('Waiting · PR #18 checks · 1h 4m')).toBeVisible();
  expect(document.querySelector('.status-cant')).toBeNull();
});

/** One of Gathr's own, as the service answers it. */
const own = (kind: Own['kind'], state: Own['state'], what: string, more: Partial<Own> = {}): Own => ({
  kind,
  state,
  what,
  name: null,
  since_us: null,
  failing: false,
  ...more,
});

test('Gathr is said like an agent, last on the line, and the slot is empty only when nobody has anything', async () => {
  // Every agent is free while Gathr waits to land a pull request.
  const land = own('pr', 'waiting', '#18 to land', { since_us: ago(4 * 60_000) * 1000 });
  service({ lines: [], agents: [free('a'), free('b'), free('c')], gathr: [land] });
  const screen = await render(<Chat />);
  await expect.poll(words).toBe('Gathr is waiting on PR #18 to land');
  expect(document.querySelector('.status-dots')).toBeNull();

  // The member works, and Gathr updates itself, lands the pull request and runs a command the lead left going.
  const update = own('update', 'working', 'Update to 0.1.240');
  const command = own('command', 'working', 'Run the test suite', { name: 'Made-up Lead' });
  service(
    { lines: [], agents: [free('a'), stands('b', 'working'), free('c')], gathr: [update, land, command] },
    { turns: [turn('now', 'b', { status: 'running', created: new Date().toISOString() })] },
  );
  await screen.rerender(<Chat key="busy" />);
  await expect.poll(words).toBe('Made-up Member and Gathr are working');

  // The member's turn ended; Gathr's last hold ends with every agent free: nothing to say.
  service({ lines: [], agents: [free('a'), free('b'), free('c')], gathr: [land] });
  await screen.rerender(<Chat key="quiet" />);
  await expect.poll(words).toBe('Gathr is waiting on PR #18 to land');
  changed({ lines: [], agents: [free('a'), free('b'), free('c')] });
  await expect.poll(words).toBe('');
});

test('the list has a row for each of Gathr’s own after the agents’, with Gathr’s mark, opening nothing', async () => {
  const hour = ago(64 * 60_000) * 1000;
  service({
    lines: [],
    agents: [
      free('a'),
      free('b'),
      stands('c', 'waiting', { on: [{ what: 'CI on #18', since_us: hour }], since_us: hour }),
    ],
    gathr: [
      own('update', 'working', 'Update to 0.1.240'),
      own('pr', 'waiting', '#18 to land', { since_us: ago(4 * 60_000) * 1000, failing: true }),
      own('command', 'working', 'Run the test suite', {
        name: 'Made-up Member',
        since_us: ago(12 * 60_000) * 1000,
      }),
    ],
  });
  await render(<Chat />);
  // Gathr works on its update: by the agents' own rule, its part comes before those who wait.
  await expect.poll(words).toBe('Gathr is working, Made-up Third is waiting');
  await page.getByRole('button', { name: 'Gathr is working, Made-up Third is waiting' }).click();
  await expect.element(list).toBeVisible();
  expect(rows()).toEqual([
    ['Made-up Third', 'Waiting · PR #18 checks · 1h 4m'],
    ['Gathr', 'Working · Update to 0.1.240'],
    ['Gathr', 'Waiting · PR #18 to land · Can’t read · 4m'],
    ['Gathr', expect.stringMatching(/^Working · Made-up Member: Run the test suite · 12m( \ds)?$/)],
  ]);
  // Gathr's mark is the one Gathr has elsewhere, and only the agent's row is a button.
  expect(document.querySelectorAll('.status-row > .face-gathr')).toHaveLength(3);
  expect(list.getByRole('button').all()).toHaveLength(1);
});
