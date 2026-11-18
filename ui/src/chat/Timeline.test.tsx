// The one timeline as the person uses it (0037): a turn's steps, Gathr's event lines, a long name line and the
// approvals docked above the composer. Talk between agents and thumbs-ups are in TeamChat.test.tsx (0072).
import { useRef } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { server, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Message, Task, ToolBody, Turn } from '../api';
import { ChatView } from './ChatView';
import { toThread } from './messages';
import { type ChatState, empty } from './state';
import { type Actions, Thread } from './Thread';

vi.mock('../live', () => ({ listen: () => () => {} }));
afterEach(() => vi.unstubAllGlobals());

const members = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' as const },
  { agent: 'b', name: 'Made-up Member', role: 'member' as const },
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
const turn = (id: string, agent: string, position: number, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  position,
  ...more,
});
const said = (id: string, turnId: string, body: string, more: Partial<Message> = {}): Message => ({
  id,
  turn: turnId,
  time: '2026-11-08T09:00:00Z',
  role: 'assistant',
  kind: 'text',
  body,
  ...more,
});
const tool = (id: string, turnId: string, name: string, more: Partial<ToolBody> = {}): Message =>
  said(id, turnId, '', { kind: 'tool', body: { tool: name, status: 'ok', ...more } });
const task = (number: number, more: Partial<Task>): Task => ({
  number,
  chat: 'c',
  giver: 'a',
  owner: 'b',
  given_in: 'lead',
  kind: 'work',
  what: `Made-up task ${number}`,
  state: 'done',
  reason: null,
  after: [],
  created_us: 0,
  ...more,
});

function View({ state, actions }: { state: ChatState; actions?: Partial<Actions> }) {
  const cache = useRef(new Map());
  const none = () => {};
  const all: Actions = {
    openTask: none,
    openAgent: none,
    diagnostics: none,
    reference: none,
    signIn: none,
    retry: none,
    resend: none,
    carryOn: none,
    approve: none,
    ...actions,
  };
  return (
    <div style={{ height: 600, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, [], cache.current).items}
        running={false}
        actions={all}
        empty={false}
      />
    </div>
  );
}

test('a turn’s steps open from its name line, with what it wrote between them, and a task it gave opens from there', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: { lead: turn('lead', 'a', 0) },
    tasks: [task(7, {})],
    messages: [
      tool('t1', 'lead', 'Read'),
      tool('t2', 'lead', 'Bash', { status: 'error', duration_ms: 27_000 }),
      said('m1', 'lead', 'Made-up remark mid-work'),
      tool('t3', 'lead', 'mcp__gathr__give_task'),
      said('m2', 'lead', 'Made-up final answer'),
    ],
  };
  const opened: number[] = [];
  const screen = await render(<View state={state} actions={{ openTask: (n) => opened.push(n) }} />);
  await expect.element(screen.getByText('Made-up final answer')).toBeVisible();
  // Folded: the remark and the steps aren't drawn.
  expect(screen.getByText('Made-up remark mid-work').query()).toBeNull();

  await screen.getByRole('button', { name: '3 steps · gave 1 task · 27s · 1 failed' }).click();
  // Each tool step is assistant-ui's Tool call; what it wrote between them and the task it gave are Gathr's lines.
  await expect.element(screen.getByText('Made-up remark mid-work')).toBeVisible();
  expect(screen.getByRole('listitem').elements()).toHaveLength(4);
  await expect.element(screen.getByRole('button', { name: /Read a file/ })).toBeVisible();
  await expect.element(screen.getByRole('img', { name: 'Failed' })).toBeVisible();
  await screen.getByRole('button', { name: '#7 Made-up task 7' }).click();
  expect(opened).toEqual([7]);
});

test('a step’s chip says what it ran, cut to the column; the step opens to it whole', async () => {
  const long = `cargo nextest run --workspace --locked ${'--made-up-flag '.repeat(12)}`.trim();
  const state: ChatState = {
    ...empty,
    members,
    turns: { lead: turn('lead', 'a', 0) },
    messages: [
      tool('t1', 'lead', 'Bash', { input: { text: 'git status --short', title: 'Show what changed' } }),
      tool('t2', 'lead', 'Shell', { input: { text: long }, duration_ms: 48_000 }),
      tool('t3', 'lead', 'Read'),
      said('m1', 'lead', 'Made-up final answer'),
    ],
  };
  const screen = await render(
    <div style={{ width: 520 }}>
      <View state={state} />
    </div>,
  );
  await screen.getByRole('button', { name: '3 steps · 48s' }).click();
  await expect.element(screen.getByRole('listitem').nth(2)).toBeVisible();
  const list = document.querySelector('.work ol') as HTMLElement;
  const chip = screen.getByText(long).element() as HTMLElement;
  expect(chip.scrollWidth, 'the long chip is cut').toBeGreaterThan(chip.clientWidth);
  expect(list.scrollWidth, 'the chip widened its column').toBeLessThanOrEqual(list.clientWidth);
  // No time per step (the owner's call, room task 142).
  expect(list.textContent).not.toContain('48s');

  await screen.getByRole('button', { name: new RegExp(`^Ran.*${long.slice(0, 20)}`) }).click();
  const whole = [...list.querySelectorAll('p')].find((p) => p.textContent === long) as HTMLElement;
  expect(whole, 'the step opens to what it ran, whole').toBeDefined();
  expect(whole.scrollWidth).toBeLessThanOrEqual(whole.clientWidth);
});

test('the keyboard opens a step: Tab reaches it from its row, Enter shows what it ran', async () => {
  const long = `cargo nextest run --workspace --locked ${'--made-up-flag '.repeat(12)}`.trim();
  const state: ChatState = {
    ...empty,
    members,
    turns: { lead: turn('lead', 'a', 0) },
    messages: [
      tool('t1', 'lead', 'Shell', { input: { text: long } }),
      said('m1', 'lead', 'Made-up final answer'),
    ],
  };
  const screen = await render(
    <div style={{ width: 520 }}>
      <View state={state} />
    </div>,
  );
  const row = screen.getByRole('button', { name: '1 step' });
  await row.click();
  const step = screen.getByRole('button', { name: new RegExp(`^Ran.*${long.slice(0, 20)}`) });
  await expect.element(step).toBeVisible();
  (row.element() as HTMLElement).focus();
  // Safari's Tab skips buttons unless the person turns that on; Option+Tab reaches them.
  await userEvent.keyboard(server.browser === 'webkit' ? '{Alt>}{Tab}{/Alt}' : '{Tab}');
  expect(document.activeElement).toBe(step.element());
  await userEvent.keyboard('{Enter}');
  await expect
    .poll(() => [...document.querySelectorAll('.work ol p')].some((p) => p.textContent === long))
    .toBe(true);
});

test('a running turn shows its steps on its name line and what the agent is doing now under it, with no bubble yet', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: { lead: turn('lead', 'a', 0, { status: 'running' }) },
    messages: [tool('t1', 'lead', 'Read'), tool('t2', 'lead', 'Shell', { status: 'running' })],
  };
  const screen = await render(<View state={state} />);
  await expect.element(screen.getByRole('button', { name: '2 steps' })).toBeVisible();
  await expect.element(screen.getByText('Running a command', { exact: true })).toBeVisible();
  expect(document.querySelector('.bubble')).toBeNull();
});

test('an event line opens in place to what more there is, and its task opens from there', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: { lead: turn('lead', 'a', 0), back: turn('back', 'a', 1, { answers: 'lead' }) },
    tasks: [task(7, {})],
    messages: [
      said('m1', 'lead', 'Made-up hand-off'),
      said(
        'g',
        'back',
        'The task you gave is settled:\n- #7 Made-up Member, Made-up task 7: done: Made-up outcome',
        {
          role: 'gathr',
        },
      ),
      said('m2', 'back', 'Made-up wrap-up'),
    ],
  };
  const opened: number[] = [];
  const screen = await render(<View state={state} actions={{ openTask: (n) => opened.push(n) }} />);
  const line = screen.getByRole('button', { name: 'Made-up Member completed: Made-up task 7' });
  await expect.element(line).toBeVisible();
  // Gathr says it, as a speaker of its own (0073).
  await expect.element(screen.getByText('Gathr', { exact: true })).toBeVisible();
  expect(screen.getByText('Task #7. Made-up outcome').query()).toBeNull();

  await line.click();
  await expect.element(screen.getByText('Task #7. Made-up outcome')).toBeVisible();
  await screen.getByRole('button', { name: 'Open task' }).click();
  expect(opened).toEqual([7]);
});

test('two answers that read the same are each the person’s own message, under the question each answers', async () => {
  const errors = vi.spyOn(console, 'error');
  const asked = {
    owner: null,
    kind: 'person' as const,
    state: 'done' as const,
    reason: 'Done',
    heard_in: 'h',
    form: 'done' as const,
  };
  const at = (s: number) => Date.parse('2026-11-08T09:00:00Z') * 1000 + s * 1_000_000;
  const state: ChatState = {
    ...empty,
    members,
    turns: { lead: turn('lead', 'a', 0), h: turn('h', 'a', 1, { answers: 'lead' }) },
    tasks: [
      task(5, { ...asked, what: 'Made-up first question', answer: { done: true, at: at(1) } }),
      task(6, { ...asked, what: 'Made-up second one', answer: { done: true, at: at(2) } }),
    ],
    messages: [
      said('m', 'lead', 'Made-up asks'),
      said('g', 'h', 'The person answered: made-up', { role: 'gathr', time: '2026-11-08T09:00:03Z' }),
      said('t', 'h', 'Made-up thanks', { time: '2026-11-08T09:00:03Z' }),
    ],
  };
  const screen = await render(<View state={state} />);
  await expect.element(screen.getByText('Made-up thanks')).toBeVisible();
  expect(screen.getByText('Done', { exact: true }).all()).toHaveLength(2);
  expect(screen.getByText('The person answered: made-up').query()).toBeNull();
  // React names messages it can't tell apart.
  expect(errors.mock.calls.filter(([said]) => String(said).includes('same key'))).toEqual([]);
  errors.mockRestore();
});
test('a member’s name line stays one line however long its task is, cut with its time in view', async () => {
  const what = `Made-up task ${'with a very long title '.repeat(12)}end`;
  const state: ChatState = {
    ...empty,
    members,
    turns: { work: turn('work', 'b', 0, { task: 7 }) },
    tasks: [task(7, { what })],
    messages: [said('m', 'work', 'Made-up finding')],
  };
  const screen = await render(<View state={state} />);
  await expect.element(screen.getByText('Made-up finding')).toBeVisible();
  const line = document.querySelector('.name-line') as HTMLElement;
  expect(line.textContent).toContain(`on ${what}`);
  // One line (20px, and the name's baseline), the task cut with … and the name whole.
  expect(line.getBoundingClientRect().height).toBeLessThan(24);
  const cut = line.querySelector('.name-line-what') as HTMLElement;
  expect(cut.scrollWidth).toBeGreaterThan(cut.clientWidth);
  const name = screen.getByText('Made-up Member', { exact: true }).element();
  expect(name.scrollWidth).toBeLessThanOrEqual(name.clientWidth);
  const column = (line.closest('.said') as HTMLElement).getBoundingClientRect();
  expect(cut.getBoundingClientRect().right).toBeLessThanOrEqual(column.right);
  expect((line.querySelector('time') as HTMLElement).getBoundingClientRect().left).toBeGreaterThan(
    name.getBoundingClientRect().right,
  );
});

test('waiting approvals dock above the composer: the oldest and how many, with Show, Deny and Allow', async () => {
  const posted: { path: string; allow: boolean }[] = [];
  const settle: ((answer: Response) => void)[] = [];
  const history = {
    chat: 'c',
    members,
    messages: [
      { id: 'u', turn: 't1', time: '', role: 'user', kind: 'text', body: 'Made-up ask' },
      ...Array.from({ length: 30 }, (_, n) =>
        said(`m${n}`, 't1', `Made-up line ${n}. ${'Made-up detail. '.repeat(30)}`),
      ),
    ],
    turns: [turn('t1', 'b', 0, { status: 'running' }), turn('t2', 'a', 1, { status: 'running' })],
    approvals: [
      { turn: 't1', request_id: 'q1', tool: 'Bash', input: { command: 'made-up --first' } },
      { turn: 't2', request_id: 'q2', tool: 'Write' },
    ],
  };
  vi.stubGlobal('fetch', (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.includes('/approvals/')) {
      posted.push({ path, allow: JSON.parse(String(init.body)).allow });
      // Answered when the test says: until then the answer is in flight.
      return new Promise<Response>((answer) => settle.push(answer));
    }
    const body = path.endsWith('/tasks') ? { tasks: [], waits: [] } : path === '/api/pauses' ? {} : history;
    return Promise.resolve(new Response(JSON.stringify(body)));
  });
  const screen = await render(
    <div style={{ height: 600, display: 'flex', flexDirection: 'column' }}>
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
      />
    </div>,
  );
  const dock = screen.getByRole('region', { name: 'Needs you' });
  await expect.element(dock).toBeVisible();
  expect(dock.element().textContent).toContain('Needs you · 1 of 2');
  expect(dock.element().textContent).toContain('Made-up Member wants to run made-up --first');

  // Show: the card where the member asked comes into view, with the keyboard on it.
  const card = screen.getByRole('group', { name: 'Allow Bash?' });
  await dock.getByRole('button', { name: 'Show' }).click();
  await expect.element(card).toHaveFocus();
  await expect.element(card).toBeInViewport();

  await dock.getByRole('button', { name: 'Deny' }).click();
  // The one being sent has left the dock; the next is the lead's, which names its tool.
  await expect.poll(() => dock.element().textContent).toContain('Needs you · 1 of 1');
  expect(dock.element().textContent).toContain('Made-up Lead wants to run Write');

  // The service refuses the first answer: it is back in the dock, the oldest again, to answer once more.
  settle[0](
    new Response(JSON.stringify({ error: 'Answer failed', why: 'made-up reason', ref: 'r1' }), {
      status: 500,
    }),
  );
  await expect.poll(() => dock.element().textContent).toContain('Needs you · 1 of 2');
  expect(dock.element().textContent).toContain('made-up --first');
  await dock.getByRole('button', { name: 'Allow' }).click();
  await expect.poll(() => dock.element().textContent).toContain('Made-up Lead wants to run Write');
  await dock.getByRole('button', { name: 'Allow' }).click();
  await expect.element(dock).not.toBeInTheDocument();
  expect(posted).toEqual([
    { path: '/api/chats/c/approvals/q1', allow: false },
    { path: '/api/chats/c/approvals/q1', allow: true },
    { path: '/api/chats/c/approvals/q2', allow: true },
  ]);
});

test('a hear-back says a task was left, or that nobody reported, in those words', async () => {
  const back = [
    'The 2 tasks you gave are settled:',
    '- #7 Made-up Member, Made-up task 7: left: the made-up half remains',
    '- #8 Made-up Member, Made-up task 8: no report: two turns ended without one',
    'Read replies with read_chat, then continue. This turn is on your task #3: end it with finish_task.',
  ];
  const state: ChatState = {
    ...empty,
    members,
    turns: { lead: turn('lead', 'a', 0), back: turn('back', 'a', 1, { answers: 'lead' }) },
    tasks: [task(7, {}), task(8, {})],
    messages: [said('m1', 'lead', 'Made-up hand-off'), said('g', 'back', back.join('\n'), { role: 'gathr' })],
  };
  const screen = await render(<View state={state} actions={{ openTask: () => {} }} />);
  const left = screen.getByRole('button', { name: 'Made-up Member left it unfinished: Made-up task 7' });
  await expect.element(left).toBeVisible();
  const silent = screen.getByRole('button', { name: 'Made-up Member gave no report: Made-up task 8' });
  await expect.element(silent).toBeVisible();
});
