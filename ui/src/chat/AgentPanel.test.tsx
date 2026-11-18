// An agent's panel as the person reaches it (0038): from Summary, the status line or an initial in the
// timeline. The service is the edge: fetch answers as it would.
import { useState } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Member, Message, Task, ToolBody, Turn } from '../api';
import type { Picked } from '../components/mention';
import type { LiveEvent } from '../live';
import { blocksOf, noneKept } from './AgentPanel';
import { ChatView } from './ChatView';
import { toThread } from './messages';
import { apply, fromHistory } from './state';
import { clock } from './team';
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
  { agent: 'd', name: 'Kit', role: 'member' },
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
const today = (h: number, m: number) => {
  const d = new Date();
  d.setHours(h, m, 0, 0);
  return d;
};
const began = today(16, 6).toISOString();
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
const task = (number: number, owner: string, more: Partial<Task> = {}): Task => ({
  number,
  chat: 'c',
  giver: 'a',
  owner,
  given_in: 'ask',
  kind: 'work',
  what: `Made-up task ${number}`,
  state: 'done',
  reason: null,
  after: [],
  created_us: today(9, 42).getTime() * 1000,
  ...more,
});
// The lead gave the member task 7 and the reviewer task 8. The member worked once, asked the lead a
// question, and is at work again; the reviewer is done; Kit has done nothing.
const history = {
  chat: 'c',
  members,
  approvals: [],
  turns: [
    turn('ask', 'a', 0),
    turn('old', 'b', 1, { task: 7 }),
    turn('rev', 'c', 2, { task: 8 }),
    turn('now', 'b', 3, { task: 7, status: 'running', created: began }),
  ],
  messages: [
    said('m1', 'ask', 'Made-up plan from the lead'),
    tool('t1', 'old', 'Read'),
    said('m2', 'old', 'Made-up earlier finding'),
    said('m3', 'old', 'Made-up question for the lead', { to: 'a' }),
    said('m4', 'rev', 'Made-up review'),
    tool('t2', 'now', 'Read'),
    tool('t3', 'now', 'Bash', { status: 'running' }),
  ],
};
const tasks = [task(7, 'b', { state: 'working' }), task(8, 'c'), task(9, 'b')];

function service(
  more: { history?: object; tasks?: Task[]; compact?: () => Response; waiting?: object } = {},
) {
  const sent: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.endsWith('/compact')) {
      sent.push(path);
      return more.compact?.() ?? new Response(JSON.stringify({ turn: 'k', status: 'queued', ref: 'k' }));
    }
    if (init?.method === 'POST' && path.endsWith('/messages')) {
      sent.push(JSON.parse(String(init.body)));
      return new Response(JSON.stringify({ turn: 'u', status: 'running' }));
    }
    // What the chat waits on (0050), where a test's words come from it.
    if (path.endsWith('/waiting')) return new Response(JSON.stringify(more.waiting ?? {}));
    const work = { tasks: more.tasks ?? tasks, waits: [] };
    const body = path.endsWith('/tasks') ? work : path === '/api/pauses' ? {} : (more.history ?? history);
    return new Response(JSON.stringify(body));
  });
  return sent;
}

// How often the stand-in workspace was drawn.
const counts = { drawn: 0 };
beforeEach(() => Object.assign(counts, { drawn: 0 }));
function Counted() {
  counts.drawn += 1;
  return null;
}

interface Shown {
  openedTasks?: number[];
  archived?: boolean;
  /** The agents the app knows: one not among them can't be sent to. */
  known?: Agent[];
  /** The stand-in workspace's width. */
  width?: number;
}

/**
 * The chat with a stand-in for the app's workspace: the open agent's panel, in the place the chat is given. In the
 * app's grid, so the top line sits above the chat as in the window.
 */
function Chat({ openedTasks = [], archived, known = agents, width = 440 }: Shown) {
  const [agent, setAgent] = useState<string>();
  const [shown, setShown] = useState(true);
  const [draft, setDraft] = useState({ text: '', picks: [] as Picked[] });
  return (
    <div className="app" style={{ height: 760 }}>
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat', archived: archived ? '2026-11-08T09:00:00Z' : null }}
        agents={known}
        draft={draft.text}
        picks={draft.picks}
        onDraft={(text, picks = []) => setDraft({ text, picks })}
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
        onOpenTask={(n) => openedTasks.push(n)}
        agentOpen={agent === undefined ? undefined : { agent, shown }}
        onOpenAgent={setAgent}
        workspace={(open) => (
          <aside aria-label="Made-up workspace" className="workspace" style={{ width }}>
            <Counted />
            <button type="button" onClick={() => setShown((s) => !s)}>
              Made-up other tab
            </button>
            {/* As the workspace keeps a tab that isn't shown: in the page, hidden. */}
            <div data-testid="tab" style={{ display: shown ? 'flex' : 'none', height: 300 }}>
              {open?.panel}
            </div>
          </aside>
        )}
      />
    </div>
  );
}

const panel = page.getByRole('complementary', { name: 'Made-up workspace' });
const who = () => panel.element().querySelector('.agent-who') as HTMLElement;
/** The header's second line, piece by piece: the state's word, what, since when. */
const stateLine = () => [...(who()?.querySelectorAll('.agent-state > *') ?? [])].map((e) => e.textContent);
/** The header as read: the name, the role after it, and the state line. */
const header = () => ({
  name: who()?.querySelector('h2')?.textContent,
  role: who()?.querySelector('h2 + span')?.textContent,
  line: stateLine(),
});
const texts = (role: 'listitem' | 'heading', within = panel) =>
  within
    .getByRole(role)
    .elements()
    .map((e) => e.textContent);

async function openFromSummary(name: RegExp) {
  await page.getByRole('button', { name: /^Summary/ }).click();
  await page.getByRole('dialog', { name: 'Summary' }).getByRole('button', { name }).click();
}

test('a working agent: when it began, its task, its running row opened, its earlier blocks and its tasks', async () => {
  service();
  const openedTasks: number[] = [];
  await render(<Chat openedTasks={openedTasks} />);
  await openFromSummary(/^Made-up Member/);
  await expect.element(panel.getByRole('heading', { name: 'Made-up Member', exact: true })).toBeVisible();
  expect(who().querySelector('h2 + span')?.textContent).toBe('Member');
  expect(stateLine()).toEqual(['Working', 'Made-up task 7', `since ${clock(began)}`]);
  expect(who().querySelector('.face .mark[data-state="working"]')).not.toBeNull();

  const current = panel.getByRole('region', { name: 'Task', exact: true });
  await expect.element(current).toHaveTextContent('TaskMade-up task 7From Made-up Lead · Open task');
  await current.getByRole('button', { name: 'Open task' }).click();
  expect(openedTasks).toEqual([7]);

  // The running turn's row, as the timeline draws it, with its steps showing.
  const now = panel.getByRole('region', { name: 'Now' });
  await expect.element(now.getByRole('button', { name: /^Working · 2 steps/ })).toBeVisible();
  // Its steps as assistant-ui's Tool calls: one done, one still running.
  expect(now.getByRole('listitem').elements()).toHaveLength(2);
  await expect.element(now.getByRole('button', { name: /Read a file/ })).toBeVisible();
  await expect.element(now.getByText('Running a command', { exact: true })).toBeVisible();

  // Newest first: what it wrote to the lead, then its earlier turn. Nobody else's blocks, nor the running one.
  const earlier = panel.getByRole('region', { name: 'Earlier in this chat' });
  const blocks = [...earlier.element().querySelectorAll('.said')].map((b) => [
    b.querySelector('.name-line-name')?.textContent,
    b.querySelector('.name-line-what')?.textContent ?? null,
    b.querySelector('.bubble')?.textContent,
  ]);
  expect(blocks).toEqual([
    ['Made-up Member', null, '@Made-up LeadMade-up question for the lead'],
    ['Made-up Member', 'on Made-up task 7', 'Made-up earlier finding'],
  ]);
  await expect.element(earlier.getByRole('button', { name: '1 step' })).toBeVisible();

  // Its tasks, open ones first; one opens in the Tasks tab.
  const list = panel.getByRole('region', { name: 'Tasks in this chat' });
  expect(texts('listitem', list)).toEqual(['Made-up task 7#7·Working', 'Made-up task 9#9']);
  await list.getByRole('button', { name: /Made-up task 9/ }).click();
  expect(openedTasks).toEqual([7, 9]);
});

test("a cancelled task is listed in its owner's tab, and a fix Gathr gave reads as Gathr's", async () => {
  service({
    tasks: [task(7, 'b', { state: 'working', by_gathr: true }), task(12, 'b', { state: 'cancelled' })],
  });
  await render(<Chat />);
  await openFromSummary(/^Made-up Member/);
  const current = panel.getByRole('region', { name: 'Task', exact: true });
  await expect.element(current).toHaveTextContent('TaskMade-up task 7From Gathr · Open task');
  const list = panel.getByRole('region', { name: 'Tasks in this chat' });
  expect(texts('listitem', list)).toEqual(['Made-up task 7#7·Working', 'Made-up task 12#12·Cancelled']);
});

test('a waiting agent says what it waits on and since when; with no task of its own it has no Task or Now', async () => {
  const earlier = new Date(2026, 10, 8, 16, 20);
  // The service says whom the lead waits for, and since when it has waited.
  const on = [
    { what: 'Made-up Member', since_us: today(9, 42).getTime() * 1000 },
    { what: 'Made-up Third', since_us: earlier.getTime() * 1000 },
  ];
  const words = 'Waiting for Made-up Member and Made-up Third';
  const lead = { agent: 'a', state: 'waiting', words, on, since_us: earlier.getTime() * 1000 };
  service({
    tasks: [...tasks, task(10, 'c', { state: 'working', created_us: earlier.getTime() * 1000 })],
    waiting: { lines: [], agents: [{ ...lead, until_us: null }] },
  });
  await render(<Chat />);
  await openFromSummary(/^Made-up Lead/);
  // A state that began on an earlier day says its date.
  const day = earlier.toLocaleDateString([], { month: 'short', day: 'numeric' });
  await expect.poll(stateLine).toEqual(['Waiting', 'Made-up Member and Made-up Third', `since ${day}`]);
  expect(who().querySelector('h2 + span')?.textContent).toBe('Supervisor');
  expect(who().querySelector('.face .mark')?.getAttribute('data-state')).toBe('waiting');
  expect(texts('heading')).toEqual(['Made-up Lead', 'Earlier in this chat']);
  await expect.element(panel.getByText('Made-up plan from the lead')).toBeVisible();
});

test('an idle agent is Free, with its earlier blocks and tasks; one with nothing in the chat says so', async () => {
  service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Third/);
  await expect.poll(header).toEqual({ name: 'Made-up Third', role: 'Reviewer', line: ['Free'] });
  expect(texts('heading')).toEqual(['Made-up Third', 'Earlier in this chat', 'Tasks in this chat']);
  expect(texts('listitem')).toEqual(['Made-up task 8#8']);

  // Another agent takes the panel's place.
  await openFromSummary(/^Kit/);
  await expect.poll(header).toEqual({ name: 'Kit', role: 'Member', line: ['Free'] });
  expect(texts('heading')).toEqual(['Kit']);
  await expect.element(panel.getByText('Nothing in this chat yet')).toBeVisible();
});

test('Message names the agent first in the message box and puts the keyboard there', async () => {
  const sent = service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Third/);
  const message = panel.getByRole('button', { name: 'Message Made-up Third' });
  await expect.element(message).toHaveTextContent('');
  await message.click();
  const input = page.getByRole('textbox', { name: 'Message' });
  await expect.element(input).toHaveFocus();
  await expect.element(input).toHaveValue('@Made-up Third ');
  await userEvent.keyboard('Made-up note{Enter}');
  // The name the button puts in counts as picked: its id is sent, and its place to draw it blue.
  const picks = [{ at: 0, len: 14, agent: 'c' }];
  await expect.poll(() => sent).toEqual([{ text: '@Made-up Third Made-up note', agent: 'c', picks }]);
});

test('an agent’s initial in the timeline opens its panel', async () => {
  service();
  await render(<Chat />);
  await page.getByRole('main').getByRole('button', { name: 'Open Made-up Third' }).click();
  await expect.element(panel.getByRole('heading', { name: 'Made-up Third', exact: true })).toBeVisible();
  // In a talk between agents too.
  await page.getByRole('main').getByRole('button', { name: 'Open Made-up Member' }).first().click();
  await expect.element(panel.getByRole('heading', { name: 'Made-up Member', exact: true })).toBeVisible();
});

test('Message is left out where it can’t be sent: an archived chat, an agent the app no longer has', async () => {
  service();
  const screen = await render(<Chat known={agents.filter((a) => a.id !== 'd')} />);
  await openFromSummary(/^Kit/);
  await expect.poll(header).toEqual({ name: 'Kit', role: 'Member', line: ['Free'] });
  expect(panel.getByRole('button', { name: /^Message/ }).query()).toBeNull();
  await openFromSummary(/^Made-up Third/);
  await expect.element(panel.getByRole('button', { name: 'Message Made-up Third' })).toBeVisible();

  await screen.rerender(<Chat archived />);
  await expect.element(page.getByText('Archived', { exact: true }).first()).toBeVisible();
  expect(panel.getByRole('button', { name: /^Message/ }).query()).toBeNull();
});

test('the context bar says New until a conversation’s first call, then its percent; a resumed one shows its fill at once', async () => {
  // The member's task opened a new conversation; the reviewer's resumed one shows its saved fill before its first
  // call; Kit has never run here.
  const shown = members.map((m) =>
    m.agent === 'b'
      ? { ...m, context: { used: null, window: 1_000_000 } }
      : m.agent === 'c'
        ? { ...m, context: { used: 300_000, window: 1_000_000 } }
        : m,
  );
  service({ history: { ...history, members: shown } });
  await render(<Chat />);
  const bar = () => who()?.querySelector('.agent-fill')?.textContent;
  await openFromSummary(/^Made-up Member/);
  await expect.poll(bar).toBe('New');
  for (const listener of live)
    listener({ type: 'context', chat: 'c', agent: 'b', context: { used: 120_000, window: 1_000_000 } });
  await expect.poll(bar).toBe('12%');

  await openFromSummary(/^Made-up Third/);
  await expect.poll(bar).toBe('30%');
  await openFromSummary(/^Kit/);
  await expect.poll(bar).toBe('New');
});

test('the header is two lines that never wrap: a long name and the longest state line at the panel’s narrowest', async () => {
  const name = 'Made-up Lead With A Name Far Too Long';
  const long = members.map((m) =>
    m.agent === 'a' ? { ...m, name, context: { used: 780_000, window: 1_000_000 } } : m,
  );
  const since_us = today(9, 42).getTime() * 1000;
  const on = ['Made-up Member', 'Made-up Third', 'CI on #18'].map((what) => ({ what, since_us }));
  const lead = { agent: 'a', state: 'waiting', words: 'Waiting', on, since_us, until_us: null };
  service({ history: { ...history, members: long }, waiting: { lines: [], agents: [lead] } });
  // At the workspace's narrowest, 360px (DESIGN.md Layout).
  await render(<Chat width={360} />);
  await openFromSummary(/^Made-up Lead/);
  await expect.element(panel.getByRole('heading', { name, exact: true })).toBeVisible();
  await expect.poll(() => who().querySelector('.face .mark')?.getAttribute('data-state')).toBe('waiting');
  const box = (e: Element) => e.getBoundingClientRect();
  const head = box(who());
  // The name line's 26px over the state line's 20px.
  const lines = [...who().children].map((e) => [e.className, box(e).top - head.top, box(e).height]);
  expect(head.height, JSON.stringify(lines)).toBe(46);
  expect(stateLine()).toEqual(['Waiting', 'Made-up Member, Made-up Third and PR #18 checks', 'since 09:42']);
  const second = [...who().querySelectorAll('.agent-state > *, .agent-fill')];
  expect(second.map((e) => Math.round(box(e).top - head.top))).toEqual([26, 26, 26, 26]);
  // The name and what it waits on give way; the role, the state's word, since when and the fill never do.
  const cut = (e: Element | null) => !!e && e.scrollWidth > e.clientWidth;
  const line = who().querySelector('.agent-state') as HTMLElement;
  expect(cut(who().querySelector('h2'))).toBe(true);
  expect(cut(line.querySelector('.name-line-what'))).toBe(true);
  expect(
    [...line.children, who().querySelector('h2 + span')].filter(
      (e) => cut(e) && !e?.querySelector('.name-line-what'),
    ),
  ).toEqual([]);
  await expect.element(panel.getByRole('button', { name: 'Context usage' })).toHaveTextContent('78%');
  // Everything stays inside the panel's padding.
  const drawn = [...who().querySelectorAll('h2, span, button')].filter((e) => box(e).width > 0);
  const parts = drawn.map((e) => ({
    what: e.getAttribute('aria-label') ?? e.textContent,
    left: Math.round(box(e).left),
    right: Math.round(box(e).right),
  }));
  const outside = parts.filter((p) => p.left < head.left - 0.5 || p.right > head.right + 0.5);
  expect(
    outside.map((p) => p.what),
    JSON.stringify({ head: [head.left, head.right], parts }),
  ).toEqual([]);
});

test("Compact, in the agent's More menu, waits as the service queues it and is offered again once it ends", async () => {
  const sent = service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Member/);
  await panel.getByRole('button', { name: 'More for Made-up Member' }).click();
  await page.getByRole('menuitem', { name: 'Compact' }).click();
  await expect.poll(() => sent).toEqual(['/api/chats/c/agents/b/compact']);
  const line = panel.getByText(/^Compaction queued/);
  await expect
    .element(line)
    .toHaveTextContent('Compaction queued · waits for its turn and background commands');
  // Asked once: the menu says so, and can't ask again.
  await panel.getByRole('button', { name: 'More for Made-up Member' }).click();
  await expect.element(page.getByRole('menuitem', { name: 'Compaction queued' })).toBeDisabled();
  await userEvent.keyboard('{Escape}');

  for (const listener of live)
    listener({ type: 'turn', chat: 'c', turn: 'k', status: 'running', ref: 'k', agent: 'b' });
  await expect.element(panel.getByText('Compacting', { exact: true })).toBeVisible();
  for (const listener of live)
    listener({ type: 'turn', chat: 'c', turn: 'k', status: 'done', ref: 'k', agent: 'b' });
  await expect.element(panel.getByText('Compacting', { exact: true })).not.toBeInTheDocument();
  await panel.getByRole('button', { name: 'More for Made-up Member' }).click();
  await expect.element(page.getByRole('menuitem', { name: 'Compact' })).toBeEnabled();
});

test('a refused Compact shows in the chat with its reference, and Compact is offered again', async () => {
  const refusal = { error: 'Compact failed', why: 'made-up: compacting already', ref: 'made-up-ref' };
  service({ compact: () => new Response(JSON.stringify(refusal), { status: 409 }) });
  await render(<Chat />);
  await openFromSummary(/^Made-up Member/);
  await panel.getByRole('button', { name: 'More for Made-up Member' }).click();
  await page.getByRole('menuitem', { name: 'Compact' }).click();
  await expect.element(page.getByRole('main').getByText('made-up: compacting already')).toBeVisible();
  await expect.element(page.getByRole('button', { name: 'Reference made-up-ref' })).toBeVisible();
  expect(panel.getByText(/^Compaction queued/).query()).toBeNull();
  await panel.getByRole('button', { name: 'More for Made-up Member' }).click();
  await expect.element(page.getByRole('menuitem', { name: 'Compact' })).toBeEnabled();
});

test('an agent that left the chat keeps its words in the timeline, with nothing to open', async () => {
  const gone = {
    ...history,
    turns: [...history.turns, turn('left', 'z', 4)],
    messages: [...history.messages, said('m9', 'left', 'Made-up words from an agent that left', { to: 'a' })],
  };
  service({ history: gone });
  await render(<Chat />);
  const words = page.getByText('Made-up words from an agent that left');
  await expect.element(words).toBeVisible();
  expect(words.element().closest('.said')?.querySelector('.said-open')).toBeNull();
  // The members' own words in the chat still open them.
  expect(document.querySelectorAll('.thread-messages .said-open').length).toBeGreaterThan(0);
});

test('a token streamed into the chat draws the workspace no more', async () => {
  service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Third/);
  await expect.poll(header).toEqual({ name: 'Made-up Third', role: 'Reviewer', line: ['Free'] });
  const before = counts.drawn;
  for (const text of ['Made-up ', 'streamed ', 'words'])
    for (const listener of live) listener({ type: 'delta', chat: 'c', turn: 'now', text });
  await expect.element(page.getByRole('main').getByText('Made-up streamed words')).toBeVisible();
  expect(counts.drawn).toBe(before);
});

test('another agent’s token leaves an agent’s blocks the very same objects; Earlier holds its latest ten', () => {
  const many = Array.from({ length: 12 }, (_, n) => n);
  const state = fromHistory({
    ...history,
    turns: [...history.turns, ...many.map((n) => turn(`c${n}`, 'c', 10 + n))],
    messages: [...history.messages, ...many.map((n) => said(`w${n}`, `c${n}`, `Made-up note ${n}`))],
  } as unknown as Parameters<typeof fromHistory>[0]);
  const cache = new Map();
  const kept = noneKept();
  const first = blocksOf(toThread(state, agents, [], cache).all, 'c', kept);
  expect(first.earlier.map((b) => (b.content as unknown as { text: string }[])[0].text)).toEqual(
    many
      .slice(2)
      .reverse()
      .map((n) => `Made-up note ${n}`),
  );

  // The member's running turn streams a word: the third agent's list and its blocks are untouched.
  const streamed = apply(state, { type: 'delta', chat: 'c', turn: 'now', text: 'Made-up word' });
  const next = blocksOf(toThread(streamed, agents, [], cache).all, 'c', kept);
  expect(next.earlier).toBe(first.earlier);
  // Its own new block makes a new list, with the blocks it had the same objects.
  const more = apply(streamed, {
    type: 'message',
    chat: 'c',
    message: said('w99', 'c11', 'Made-up afterthought'),
  });
  const grown = blocksOf(toThread(more, agents, [], cache).all, 'c', kept);
  expect(grown.earlier).not.toBe(first.earlier);
  expect(grown.earlier[1]).toBe(first.earlier[1]);
});

test('behind another tab the panel keeps its scroll and its opened rows, and nothing draws it until it shows again', async () => {
  service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Member/);
  const earlier = panel.getByRole('button', { name: '1 step' });
  await earlier.click();
  await expect.element(earlier).toHaveAttribute('aria-expanded', 'true');
  const scroller = panel.element().querySelector('.agent-panel') as HTMLElement;
  // Once the row has finished opening, so nothing but the tab switch can move the scroll.
  const opening = scroller
    .getAnimations({ subtree: true })
    .filter((a) => a.effect?.getComputedTiming().endTime !== Number.POSITIVE_INFINITY);
  await Promise.all(opening.map((a) => a.finished));
  scroller.scrollTop = 150;
  expect(scroller.scrollTop).toBe(150);
  const other = page.getByRole('button', { name: 'Made-up other tab' });
  await other.click();
  expect(page.getByTestId('tab').element().style.display).toBe('none');
  await other.click();
  expect(panel.element().querySelector('.agent-panel')).toBe(scroller);
  expect(scroller.scrollTop).toBe(150);
  await expect.element(earlier).toHaveAttribute('aria-expanded', 'true');

  // Hidden, even its own agent's next step doesn't draw it: it reads as it was last shown.
  await other.click();
  const next = tool('t9', 'now', 'Edit', { status: 'running' });
  for (const listener of live) listener({ type: 'message', chat: 'c', message: next });
  // A running row shows its latest step under "Working".
  await expect.element(page.getByRole('main').getByText('Editing a file', { exact: true })).toBeVisible();
  expect(scroller.querySelector('[data-slot="run-activity"][data-status="running"]')?.textContent).toContain(
    'Running a command',
  );

  // Shown again, it catches up, the same panel as before.
  await other.click();
  await expect.element(panel.getByText('Editing a file', { exact: true })).toBeVisible();
  expect(panel.element().querySelector('.agent-panel')).toBe(scroller);
  await expect.element(earlier).toHaveAttribute('aria-expanded', 'true');
});
