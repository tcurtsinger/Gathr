// The team column hidden and shown again keeps the reader's place (owner's README, Interactions).
import { useState } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import '../theme.css';
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
const long = 'Made-up finding. '.repeat(60);
const reply = (n: number) => ({
  id: `r${n}`,
  turn: `w${n}`,
  time: '',
  role: 'assistant',
  kind: 'text',
  body: long,
});
const work = (n: number) => ({
  id: `w${n}`,
  status: 'done',
  why: null,
  ref: `w${n}`,
  agent: 'b',
  task: n,
  position: n,
});
const tasks = Array.from({ length: 13 }, (_, n) => ({
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
// The lead handed out twelve tasks; each member reply is long, so the team column scrolls. So does the
// lead's reply, in numbered paragraphs that wrap again when the column's width changes.
const lines = Array.from({ length: 40 }, (_, n) => `Made-up line ${n}. ${'Made-up plan detail. '.repeat(8)}`);
const history = {
  chat: 'c',
  messages: [
    { id: 'u', turn: 'm', time: '', role: 'user', kind: 'text', body: 'Made-up ask' },
    { id: 'l', turn: 'm', time: '', role: 'assistant', kind: 'text', body: lines.join('\n\n') },
    ...Array.from({ length: 12 }, (_, n) => reply(n)),
  ],
  turns: [
    { id: 'm', status: 'done', why: null, ref: 'm', agent: 'a', position: -1 },
    ...Array.from({ length: 12 }, (_, n) => work(n)),
  ],
  approvals: [],
  members: [
    { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
    { agent: 'b', name: 'Made-up Member', role: 'member' },
  ],
};

function Chat() {
  const [split, setSplit] = useState(true);
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
        split={split}
        onSplit={setSplit}
        plan={false}
        onPlan={() => {}}
      />
    </div>
  );
}

async function open(chat = history) {
  vi.stubGlobal('fetch', async (path: string) =>
    json(
      path.endsWith('/tasks')
        ? { tasks, waits: [] }
        : path === '/api/pauses'
          ? { pauses: [], usage: [] }
          : chat,
    ),
  );
  const screen = await render(<Chat />);
  const team = screen.getByRole('region', { name: 'Team' });
  await expect.element(team.getByText('Made-up finding.', { exact: false }).nth(11)).toBeInTheDocument();
  const viewport = team.element().querySelector('.thread-viewport') as HTMLElement;
  // Opened at the latest reply.
  await expect.poll(() => viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop).toBeLessThan(2);
  const toggle = async () => {
    await screen.getByRole('button', { name: /team chat/ }).click();
    // Two frames: the autoscroll and size observers have run.
    await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
  };
  const main = screen.getByRole('region', { name: 'Made-up Lead' }).element();
  return { screen, viewport, toggle, main: main.querySelector('.thread-viewport') as HTMLElement };
}

test('Hide then Show keeps the line being read in the main column where it was, as the column widens and narrows', async () => {
  const { main, toggle } = await open();
  main.scrollTo({ top: (main.scrollHeight - main.clientHeight) * 0.6, behavior: 'instant' });
  await expect.poll(() => main.scrollTop).toBeGreaterThan(100);
  // The first numbered paragraph below the sticky label, and how far down the column it starts.
  const top = () => main.getBoundingClientRect().top;
  const read = [...main.querySelectorAll('p')].find(
    (p) => p.getBoundingClientRect().top >= top() + 56,
  ) as HTMLElement;
  const offset = () => read.getBoundingClientRect().top - top();
  const was = offset();

  await toggle();
  expect(Math.abs(offset() - was)).toBeLessThan(2);
  await toggle();
  expect(Math.abs(offset() - was)).toBeLessThan(2);
});

test('Hide then Show keeps the team column where the reader left it', async () => {
  const { screen, viewport, toggle } = await open();
  viewport.scrollTo({ top: viewport.scrollHeight / 3, behavior: 'instant' });
  await expect.poll(() => viewport.scrollTop).toBeGreaterThan(100);
  const reading = viewport.scrollTop;

  await toggle();
  // Kept drawn, but out of reach of the keyboard and screen readers.
  await expect.element(screen.getByRole('region', { name: 'Team' })).not.toBeInTheDocument();
  await toggle();
  expect(viewport.scrollTop).toBe(reading);
});

test('a chat whose team shrank to its lead still shows the hand-offs it saved, and can hide them', async () => {
  const { screen, toggle } = await open({ ...history, members: history.members.slice(0, 1) });
  await toggle();
  await expect.element(screen.getByRole('region', { name: 'Team' })).not.toBeInTheDocument();
  await expect.element(screen.getByRole('button', { name: 'Show team chat' })).toBeVisible();
});

test('a team column following the latest reply follows one that came while it was hidden', async () => {
  const { viewport, toggle } = await open();
  await toggle();
  const message = { ...reply(12), body: 'Made-up late reply' };
  for (const send of live) {
    send({ type: 'turn', chat: 'c', turn: 'w12', status: 'done', agent: 'b', task: 12, position: 12 });
    send({ type: 'message', chat: 'c', message } as LiveEvent);
  }
  await toggle();
  await expect.poll(() => viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop).toBeLessThan(2);
});
