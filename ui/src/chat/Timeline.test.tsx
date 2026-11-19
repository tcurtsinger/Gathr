// The one timeline as the person uses it (0037): a turn's runs of steps, Gathr's event lines, a long name line in the
// agent's panel, and what a needs-you row in Summary goes to when an approval waits. Talk between agents and
// thumbs-ups are in TeamChat.test.tsx (0072); how a turn's words and runs read is in turn.test.tsx.
import { useRef } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { page, server, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Message, Task, ToolBody, Turn } from '../api';
import { ChatView } from './ChatView';
import { opened as everyThread } from './madeUp';
import { toThread } from './messages';
import { type ChatState, empty } from './state';
import { type Actions, Blocks, Thread } from './Thread';

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

/** The chat; with `panel`, its blocks as the agent's panel draws them (`InPanel`). */
function View({ state, actions, panel }: { state: ChatState; actions?: Partial<Actions>; panel?: boolean }) {
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
  const timeline = toThread(state, agents, [], cache.current, everyThread);
  if (panel) return <Blocks messages={timeline.every} actions={all} />;
  return (
    <div style={{ height: 600, display: 'flex' }}>
      <Thread messages={timeline.items} running={false} actions={all} empty={false} />
    </div>
  );
}

test('in the agent’s panel a turn reads as in the chat: its message, one line opening to its steps and narration, a task it gave opening', async () => {
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
  const screen = await render(<View state={state} actions={{ openTask: (n) => opened.push(n) }} panel />);
  await expect.element(screen.getByText('Made-up final answer')).toBeVisible();
  // Closed, its steps and narration are one line (0082): no step is drawn, nor the remark.
  expect(screen.getByRole('listitem').elements()).toHaveLength(0);
  expect(screen.getByText('Made-up remark mid-work').query()).toBeNull();

  await screen.getByRole('button', { name: 'Read 1 file, ran 1 command, gave 1 task' }).click();
  // Each tool step is assistant-ui's Tool call; the remark and the task it gave are Gathr's lines between them.
  expect(screen.getByRole('listitem').elements()).toHaveLength(4);
  await expect.element(screen.getByRole('button', { name: /Read a file/ })).toBeVisible();
  await expect.element(screen.getByRole('img', { name: 'Failed' })).toBeVisible();
  await expect.element(screen.getByText('Made-up remark mid-work')).toBeVisible();
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
      <View state={state} panel />
    </div>,
  );
  await screen.getByRole('button', { name: 'Ran 2 commands, read 1 file' }).click();
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
      <View state={state} panel />
    </div>,
  );
  const row = screen.getByRole('button', { name: 'Ran 1 command' });
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
test('in the agent’s panel a member’s name line stays one line however long its task is, cut with its time in view', async () => {
  const what = `Made-up task ${'with a very long title '.repeat(12)}end`;
  const state: ChatState = {
    ...empty,
    members,
    turns: { work: turn('work', 'b', 0, { task: 7 }) },
    tasks: [task(7, { what })],
    messages: [said('m', 'work', 'Made-up finding')],
  };
  const screen = await render(<View state={state} panel />);
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

test('waiting approvals: Summary says who needs you, and a press on the row goes to the oldest where it was asked', async () => {
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
  vi.stubGlobal('fetch', (path: string) => {
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
  // Said by the faces' marks and Summary: nothing above the message box says it.
  const summary = screen.getByRole('button', { name: 'Summary: Made-up Lead and Made-up Member need you' });
  await expect.element(summary).toBeVisible();
  expect(document.querySelector('[aria-label="Needs you"]')).toBeNull();
  expect(document.querySelector('.thread-footer')?.textContent).not.toContain('need');

  // The card where the member asked, the oldest, comes into view with the keyboard on it.
  const card = screen.getByRole('group', { name: 'Allow Bash?' });
  await summary.click();
  await page
    .getByRole('dialog', { name: 'Summary' })
    .getByRole('button', { name: /^Made-up Member / })
    .click();
  await expect.element(card).toHaveFocus();
  await expect.element(card).toBeInViewport();
});
