// An agent's panel as the person reaches it (0038): from Summary, the Working line or an initial in the
// timeline. The service is the edge: fetch answers as it would.
import { useState } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Member, Message, Task, ToolBody, Turn } from '../api';
import { ChatView } from './ChatView';
import { clock } from './team';

vi.mock('../live', () => ({ listen: () => () => {} }));
beforeEach(() => page.viewport(1280, 800));
afterEach(() => vi.unstubAllGlobals());

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

function service() {
  const sent: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.endsWith('/messages')) {
      sent.push(JSON.parse(String(init.body)));
      return new Response(JSON.stringify({ turn: 'u', status: 'running' }));
    }
    const body = path.endsWith('/tasks') ? { tasks, waits: [] } : path === '/api/pauses' ? {} : history;
    return new Response(JSON.stringify(body));
  });
  return sent;
}

/** The chat with a stand-in for the app's workspace: the open agent's name, then its panel. */
function Chat({ openedTasks = [] }: { openedTasks?: number[] }) {
  const [agent, setAgent] = useState<string>();
  const [draft, setDraft] = useState('');
  return (
    <div style={{ height: 760, display: 'flex' }}>
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat' }}
        agents={agents}
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
        agentOpen={agent}
        onOpenAgent={setAgent}
        workspace={(open) =>
          open && (
            <aside aria-label="Made-up workspace" style={{ display: 'flex', width: 440 }}>
              {open.panel}
            </aside>
          )
        }
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
  service();
  await render(<Chat />);
  await openFromSummary(/^Made-up Lead/);
  await expect.poll(() => who().textContent).toContain('Supervisor · Waiting for Made-up Member since 09:42');
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
  await userEvent.keyboard('Made-up note{Enter}');
  await expect.poll(() => sent).toEqual([{ text: 'Made-up note', agent: 'c' }]);
});

test('the Working line opens Summary and closes it; Escape gives focus back to the line', async () => {
  service();
  await render(<Chat />);
  const line = page.getByRole('button', { name: 'Working Made-up Member' });
  const summary = page.getByRole('dialog', { name: 'Summary' });
  await line.click();
  await expect.element(summary).toBeVisible();
  await userEvent.keyboard('{Escape}');
  await expect.element(summary).not.toBeInTheDocument();
  await expect.element(line).toHaveFocus();

  await line.click();
  await expect.element(summary).toBeVisible();
  await line.click();
  await expect.element(summary).not.toBeInTheDocument();
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
