// The one timeline as the person uses it (0037): a turn's row, talk between agents, Gathr's event lines
// and the approvals docked above the composer.
import { useRef } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
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

test('a turn’s row opens to its steps and what it wrote between them, and a task it gave opens from there', async () => {
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

  await screen.getByRole('button', { name: 'Worked for 27s · 3 steps · gave 1 task' }).click();
  const steps = screen.getByRole('listitem');
  await expect.element(steps.nth(3)).toBeVisible();
  expect(steps.elements().map((li) => li.textContent)).toEqual([
    'Read a file',
    'Ran a commandFailed27s',
    'Made-up remark mid-work',
    'Gave Made-up Member a task#7 Made-up task 7',
  ]);
  await screen.getByRole('button', { name: '#7 Made-up task 7' }).click();
  expect(opened).toEqual([7]);
});

test('a step’s chip says what it ran; a long one is cut to the column and shows whole when pointed at', async () => {
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
  await screen.getByRole('button', { name: 'Worked for 48s · 3 steps' }).click();
  const steps = screen.getByRole('listitem');
  await expect.element(steps.nth(2)).toBeVisible();
  expect(steps.elements().map((li) => li.textContent)).toEqual([
    'Show what changedgit status --short',
    `Ran${long}48s`,
    'Read a file',
  ]);

  const list = document.querySelector('.work-steps') as HTMLElement;
  const [short, cut] = [...list.querySelectorAll<HTMLElement>('.work-detail')];
  expect(short.scrollWidth, 'a short chip shows whole').toBeLessThanOrEqual(short.clientWidth);
  expect(cut.scrollWidth, 'the long chip is cut').toBeGreaterThan(cut.clientWidth);
  expect(list.scrollWidth, 'the chip widened its column').toBeLessThanOrEqual(list.clientWidth);
  const row = cut.closest('li') as HTMLElement;
  const time = row.querySelector('.work-time') as HTMLElement;
  expect(time.getBoundingClientRect().right).toBeLessThanOrEqual(list.getBoundingClientRect().right);

  // Only a chip that's cut has more to show.
  await screen.getByText('git status --short').hover();
  await screen.getByText(long).hover();
  await expect.element(screen.getByRole('tooltip')).toHaveTextContent(long);
  expect(screen.getByRole('tooltip').elements()).toHaveLength(1);
});

test('a running turn’s row says what the agent is doing now, with no message under it yet', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: { lead: turn('lead', 'a', 0, { status: 'running' }) },
    messages: [tool('t1', 'lead', 'Read'), tool('t2', 'lead', 'Shell', { status: 'running' })],
  };
  const screen = await render(<View state={state} />);
  await expect.element(screen.getByRole('button', { name: /^Running a command · 2 steps/ })).toBeVisible();
  expect(document.querySelector('.work-row .shimmer')?.textContent).toBe('Running a command');
  expect(document.querySelector('.reply-body')?.textContent).toBe('');
});

test('agents’ messages to each other read as one block, each headed by who wrote to whom', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: { ask: turn('ask', 'b', 0), told: turn('told', 'a', 1, { answers: 'ask' }) },
    messages: [
      said('q', 'ask', 'Made-up question', { to: 'a' }),
      said('copy', 'told', 'Made-up Member wrote to you: Made-up question', { role: 'gathr', copy_of: 'q' }),
      said('r', 'told', 'Made-up answer', { to: 'b' }),
    ],
  };
  const screen = await render(<View state={state} />);
  await expect.element(screen.getByText('Made-up answer')).toBeVisible();
  const blocks = document.querySelectorAll('.exchange');
  expect(blocks).toHaveLength(1);
  const heads = [...blocks[0].querySelectorAll('[data-slot="speaker-identity"]')].map((h) => h.textContent);
  expect(heads).toEqual(['MLMade-up Leadto Made-up Member', 'MMMade-up Memberto Made-up Lead'].reverse());
  // Gathr's copy of the question in the receiver's turn isn't drawn.
  expect(screen.getByText(/wrote to you/).query()).toBeNull();
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
  // No "Gathr" speaker: the line is the whole of it.
  expect(screen.getByText('Gathr', { exact: true }).query()).toBeNull();
  expect(screen.getByText('Task #7. Made-up outcome').query()).toBeNull();

  await line.click();
  await expect.element(screen.getByText('Task #7. Made-up outcome')).toBeVisible();
  await screen.getByRole('button', { name: 'Open task' }).click();
  expect(opened).toEqual([7]);
});

test('two event lines that read the same open one at a time, each to its own question', async () => {
  const errors = vi.spyOn(console, 'error');
  const asked = {
    owner: null,
    kind: 'person' as const,
    state: 'done' as const,
    reason: 'Done',
    heard_in: 'h',
  };
  const state: ChatState = {
    ...empty,
    members,
    turns: { lead: turn('lead', 'a', 0), h: turn('h', 'a', 1, { answers: 'lead' }) },
    tasks: [
      task(5, { ...asked, what: 'Made-up first question' }),
      task(6, { ...asked, what: 'Made-up second one' }),
    ],
    messages: [
      said('g', 'h', 'The person answered: made-up', { role: 'gathr' }),
      said('m', 'h', 'Made-up thanks'),
    ],
  };
  const screen = await render(<View state={state} />);
  const lines = screen.getByRole('button', { name: 'You answered Made-up Lead: Done' });
  await expect.element(lines.nth(1)).toBeVisible();
  await lines.nth(1).click();
  await expect.element(screen.getByText('Question #6: Made-up second one')).toBeVisible();
  expect(screen.getByText('Question #5: Made-up first question').query()).toBeNull();
  // React names lines it can't tell apart.
  expect(errors.mock.calls.filter(([said]) => String(said).includes('same key'))).toEqual([]);
  errors.mockRestore();
});

test('a member’s header stays one line however long its task is, with the time in view', async () => {
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
  const header = document.querySelector('.speaker') as HTMLElement;
  expect(header.textContent).toContain(`on ${what}`);
  // One line: the 24px avatar is its tallest part.
  expect(header.getBoundingClientRect().height).toBeLessThanOrEqual(28);
  const name = screen.getByText('Made-up Member', { exact: true }).element();
  expect(name.scrollWidth).toBeLessThanOrEqual(name.clientWidth);
  const column = (header.closest('.reply') as HTMLElement).getBoundingClientRect();
  expect((header.querySelector('time') as HTMLElement).getBoundingClientRect().right).toBeLessThanOrEqual(
    column.right,
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
