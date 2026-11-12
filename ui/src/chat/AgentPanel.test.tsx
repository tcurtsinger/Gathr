// An agent's panel as the person reaches it (0038): from Summary, the Working line or an initial in the
// timeline. The service is the edge: fetch answers as it would.
import { useState } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Member, Message, Task, ToolBody, Turn } from '../api';
import type { LiveEvent } from '../live';
import { blocksOf, noneKept } from './AgentPanel';
import { ChatView } from './ChatView';
import { toThread } from './messages';
import { apply, fromHistory } from './state';
import { clock } from './team';

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

function service(more: { history?: object; tasks?: Task[]; compact?: () => Response } = {}) {
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
    const work = { tasks: more.tasks ?? tasks, waits: [] };
    const body = path.endsWith('/tasks') ? work : path === '/api/pauses' ? {} : (more.history ?? history);
    return new Response(JSON.stringify(body));
  });
  return sent;
}

// How often the stand-in workspace was drawn, and how often the chat asked it to close.
const counts = { drawn: 0, closed: 0 };
beforeEach(() => Object.assign(counts, { drawn: 0, closed: 0 }));
function Counted() {
  counts.drawn += 1;
  return null;
}

interface Shown {
  openedTasks?: number[];
  archived?: boolean;
  /** The agents the app knows: one not among them can't be sent to. */
  known?: Agent[];
}

/** The chat with a stand-in for the app's workspace: the open agent's panel, in the place the chat is given. */
function Chat({ openedTasks = [], archived, known = agents }: Shown) {
  const [agent, setAgent] = useState<string>();
  const [shown, setShown] = useState(true);
  const [draft, setDraft] = useState('');
  return (
    <div style={{ height: 760, display: 'flex' }}>
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat', archived: archived ? '2026-11-08T09:00:00Z' : null }}
        agents={known}
        onEditAgent={() => {}}
        draft={draft}
        onDraft={setDraft}
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
        onCloseWorkspace={() => {
          counts.closed += 1;
        }}
        workspace={(open) => (
          <aside aria-label="Made-up workspace" style={{ width: 440 }}>
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
  expect(who().textContent).toContain(`Member · Working since ${clock(began)}`);
  expect(who().querySelector('.initial[data-busy]')).not.toBeNull();

  const current = panel.getByRole('region', { name: 'Task', exact: true });
  await expect.element(current).toHaveTextContent('TaskMade-up task 7From Made-up Lead · Open task');
  await current.getByRole('button', { name: 'Open task' }).click();
  expect(openedTasks).toEqual([7]);

  // The running turn's row, as the timeline draws it, with its steps showing.
  const now = panel.getByRole('region', { name: 'Now' });
  await expect.element(now.getByRole('button', { name: /^Running a command · 2 steps/ })).toBeVisible();
  expect(texts('listitem', now)).toEqual(['Read a file', 'Running a command']);

  // Newest first: what it wrote to the lead, then its earlier turn. Nobody else's blocks, nor the running one.
  const earlier = panel.getByRole('region', { name: 'Earlier in this chat' });
  const blocks = [...earlier.element().querySelectorAll('.reply')].map((b) => [
    b.querySelector('[data-slot="speaker-identity"]')?.textContent,
    b.querySelector('.reply-body')?.textContent,
  ]);
  expect(blocks).toEqual([
    ['MMMade-up Memberto Made-up Lead', 'Made-up question for the lead'],
    ['MMMade-up Memberon Made-up task 7', 'Made-up earlier finding'],
  ]);
  await expect.element(earlier.getByRole('button', { name: 'Worked · 1 step' })).toBeVisible();

  // Its tasks, open ones first; one opens in the Tasks tab.
  const list = panel.getByRole('region', { name: 'Tasks in this chat' });
  expect(
    list
      .getByRole('listitem')
      .elements()
      .map((li) => [li.dataset.status, li.textContent]),
  ).toEqual([
    ['active', 'activeMade-up task 7'],
    ['done', 'doneMade-up task 9'],
  ]);
  await list.getByRole('button', { name: 'Made-up task 9' }).click();
  expect(openedTasks).toEqual([7, 9]);
});

test('a waiting agent says who it waits for and since when; with no task of its own it has no Task or Now', async () => {
  const earlier = new Date(2026, 10, 8, 16, 20);
  service({ tasks: [...tasks, task(10, 'c', { state: 'working', created_us: earlier.getTime() * 1000 })] });
  await render(<Chat />);
  await openFromSummary(/^Made-up Lead/);
  // A wait that began today says its time; one from an earlier day, its date.
  const day = earlier.toLocaleDateString([], { month: 'short', day: 'numeric' });
  await expect
    .poll(() => who().textContent)
    .toContain(`Supervisor · Waiting for Made-up Member since 09:42 and Made-up Third since ${day}`);
  expect(who().querySelector('.initial[data-busy]')).toBeNull();
  expect(texts('heading')).toEqual(['Made-up Lead', 'Earlier in this chat']);
  await expect.element(panel.getByText('Made-up plan from the lead')).toBeVisible();
});

test('an idle agent is Ready, with its earlier blocks and tasks; one with nothing in the chat says so', async () => {
  service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Third/);
  await expect.poll(() => who().textContent).toContain('Reviewer · Ready');
  expect(texts('heading')).toEqual(['Made-up Third', 'Earlier in this chat', 'Tasks in this chat']);
  expect(texts('listitem')).toEqual(['doneMade-up task 8']);

  // Another agent takes the panel's place.
  await openFromSummary(/^Kit/);
  await expect.poll(() => who().textContent).toContain('Member · Ready');
  expect(texts('heading')).toEqual(['Kit']);
  await expect.element(panel.getByText('Nothing in this chat yet')).toBeVisible();
});

test('Message picks the agent as the recipient and puts the keyboard in the composer', async () => {
  const sent = service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Third/);
  await panel.getByRole('button', { name: 'Message Made-up Third' }).click();
  await expect.element(page.getByRole('textbox', { name: 'Message' })).toHaveFocus();
  await expect.element(page.getByRole('combobox', { name: 'Agent' })).toHaveTextContent('Made-up Third');
  // Wide enough for the workspace to sit beside the chat: it stays open.
  expect(counts.closed).toBe(0);
  await userEvent.keyboard('Made-up note{Enter}');
  await expect.poll(() => sent).toEqual([{ text: 'Made-up note', agent: 'c' }]);
});

test('the Working line opens Summary and closes it; Escape gives focus back to the line', async () => {
  service();
  await render(<Chat />);
  const line = page.getByRole('button', { name: 'Working Made-up Member' });
  const summary = page.getByRole('dialog', { name: 'Summary' });
  await expect.element(line).toHaveAttribute('aria-haspopup', 'dialog');
  await expect.element(line).toHaveAttribute('aria-expanded', 'false');
  // Who is working is announced from beside the button, never by a live region around the control.
  const status = page.getByRole('status').filter({ hasText: 'Working' });
  await expect.element(status).toHaveTextContent('Working: Made-up Member');
  expect(status.element().contains(line.element())).toBe(false);
  await line.click();
  await expect.element(summary).toBeVisible();
  await expect.element(line).toHaveAttribute('aria-expanded', 'true');
  await userEvent.keyboard('{Escape}');
  await expect.element(summary).not.toBeInTheDocument();
  await expect.element(line).toHaveFocus();

  await line.click();
  await expect.element(summary).toBeVisible();
  await line.click();
  await expect.element(summary).not.toBeInTheDocument();

  // Opened from the line and closed with the header's button: focus stays with the button last used.
  await line.click();
  await expect.element(summary).toBeVisible();
  const button = page.getByRole('button', { name: /^Summary/ });
  await button.click();
  await expect.element(summary).not.toBeInTheDocument();
  await expect.element(button).toHaveFocus();
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
  await expect.poll(() => who().textContent).toContain('Member · Ready');
  expect(panel.getByRole('button', { name: /^Message/ }).query()).toBeNull();
  await openFromSummary(/^Made-up Third/);
  await expect.element(panel.getByRole('button', { name: 'Message Made-up Third' })).toBeVisible();

  await screen.rerender(<Chat archived />);
  await expect.element(page.getByText('Archived', { exact: true }).first()).toBeVisible();
  expect(panel.getByRole('button', { name: /^Message/ }).query()).toBeNull();
});

test('where the workspace lies over the chat, Message closes it so the composer is in reach', async () => {
  await page.viewport(1000, 800);
  service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Third/);
  await panel.getByRole('button', { name: 'Message Made-up Third' }).click();
  expect(counts.closed).toBe(1);
  await expect.element(page.getByRole('textbox', { name: 'Message' })).toHaveFocus();
});

test("an agent's context shows in its panel, from the engine's fill and window; none before its first call", async () => {
  const filled = members.map((m) =>
    m.agent === 'b' ? { ...m, context: { used: 250_000, window: 1_000_000 } } : m,
  );
  service({ history: { ...history, members: filled } });
  await render(<Chat />);
  await openFromSummary(/^Made-up Member/);
  await expect.element(panel.getByRole('button', { name: 'Context usage' })).toHaveTextContent('250k (25%)');
  await openFromSummary(/^Made-up Third/);
  await expect.poll(() => who().textContent).toContain('Reviewer · Ready');
  expect(panel.getByRole('button', { name: 'Context usage' }).query()).toBeNull();
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
  expect(words.element().closest('.reply')?.querySelector('.speaker-open')).toBeNull();
  // The members' own words in the timeline still open them.
  expect(document.querySelectorAll('.exchange .speaker-open').length).toBe(1);
});

test('a token streamed into the chat draws the workspace no more', async () => {
  service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Third/);
  await expect.poll(() => who().textContent).toContain('Reviewer · Ready');
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
  const first = blocksOf(toThread(state, agents, [], cache).items, 'c', kept);
  expect(first.earlier.map((b) => (b.content as unknown as { text: string }[])[0].text)).toEqual(
    many
      .slice(2)
      .reverse()
      .map((n) => `Made-up note ${n}`),
  );

  // The member's running turn streams a word: the third agent's list and its blocks are untouched.
  const streamed = apply(state, { type: 'delta', chat: 'c', turn: 'now', text: 'Made-up word' });
  const next = blocksOf(toThread(streamed, agents, [], cache).items, 'c', kept);
  expect(next.earlier).toBe(first.earlier);
  // Its own new block makes a new list, with the blocks it had the same objects.
  const more = apply(streamed, {
    type: 'message',
    chat: 'c',
    message: said('w99', 'c11', 'Made-up afterthought'),
  });
  const grown = blocksOf(toThread(more, agents, [], cache).items, 'c', kept);
  expect(grown.earlier).not.toBe(first.earlier);
  expect(grown.earlier[1]).toBe(first.earlier[1]);
});

test('behind another tab the panel keeps its scroll and its opened rows, and nothing draws it until it shows again', async () => {
  service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Member/);
  const earlier = panel.getByRole('button', { name: 'Worked · 1 step' });
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
  await expect.element(page.getByRole('main').getByRole('button', { name: /^Editing a file/ })).toBeVisible();
  expect(scroller.querySelector('.work-row[data-running]')?.textContent).toContain('Running a command');

  // Shown again, it catches up, the same panel as before.
  await other.click();
  await expect.element(panel.getByRole('button', { name: /^Editing a file/ })).toBeVisible();
  expect(panel.element().querySelector('.agent-panel')).toBe(scroller);
  await expect.element(earlier).toHaveAttribute('aria-expanded', 'true');
});
