// A turn's block in the chat as Claude's app reads it (the owner's call of 2026-11-19): inside the agent's one bubble,
// its words where it wrote them, each run of steps between them one quiet line, and the working line while it runs.
// When the turn ends nothing moves.
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message } from '../api';
import { agents, alone, custom, opened, round, said, task, tool } from './madeUp';
import { toThread } from './messages';
import { noActions } from './recorded';
import type { ChatState } from './state';
import { Thread } from './Thread';

function View({ state }: { state: ChatState }) {
  const cache = useRef(new Map());
  return (
    <div style={{ height: 900, width: 900, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, [], cache.current, opened).items}
        running={false}
        actions={noActions}
        empty={false}
      />
    </div>
  );
}

const bubbleOf = (agent: string) =>
  document.querySelector(`.thread-messages .said[data-agent="${agent}"] .bubble`);

/** What an agent's bubble reads, piece by piece: its words, and "> " before each run's line ("Working" while it runs). */
const read = (agent: string) =>
  [...(bubbleOf(agent)?.children ?? [])].map((piece) => {
    if (!piece.matches('.work')) return piece.textContent?.trim();
    const line = piece.querySelector('[data-slot="collapsible-trigger"], p')?.textContent ?? '';
    return `> ${line.replace(/\s*·.*$/, '').trim()}`;
  });

/** Rowan's turn on task 1, a reply in the ask's thread, with `msgs` as all it wrote so far. */
function rowan(msgs: Message[], status: 'running' | 'done' | 'queued' = 'done', one = false): ChatState {
  const state = round();
  for (const id of ['rowan', 'kit']) state.turns[id] = { ...state.turns[id], topic: state.turns.ask.topic };
  state.turns.rowan = { ...state.turns.rowan, status };
  state.messages = [
    ...state.messages.filter((m) => !(m.turn === 'rowan' && m.role === 'assistant')),
    ...msgs,
  ];
  return one ? alone(state) : state;
}

const words = (text: string) => said('rowan', 'assistant', text);

test('a running turn with no message yet draws its agent’s bubble with the working line; a queued one draws nothing', async () => {
  const screen = await render(<View state={rowan([], 'running')} />);
  await expect.poll(() => read('rowan')).toEqual(['> Working']);
  expect(bubbleOf('rowan')?.closest('.said')?.querySelector('.name-line-name')?.textContent).toBe('Rowan');

  await screen.rerender(<View state={rowan([], 'queued')} />);
  await expect.poll(() => bubbleOf('rowan')).toBeNull();
});

test('words, then a tool call while the turn runs: the words stay in the chat, the working line under them', async () => {
  const msgs = [words('I’ll read the plan first.'), tool('rowan', 'r1', 'Read', { status: 'running' })];
  await render(<View state={rowan(msgs, 'running')} />);
  await expect.poll(() => read('rowan')).toEqual(['I’ll read the plan first.', '> Working']);
  // Its latest step, under the working line.
  await expect.element(page.getByText('Reading a file')).toBeVisible();
});

/** Words, three steps, words, three commands (the last still running unless `ended`), then final words once ended. */
function worked(ended: boolean) {
  return [
    words('I’ll read the plan first.'),
    tool('rowan', 's1', 'Read'),
    tool('rowan', 's2', 'Read'),
    tool('rowan', 's3', 'Bash'),
    words('The plan misses one case. Checking it.'),
    tool('rowan', 's4', 'Bash'),
    tool('rowan', 's5', 'Bash'),
    tool('rowan', 's6', 'Bash', { status: ended ? 'ok' : 'running' }),
    ...(ended ? [words('Done. The plan needs one more part.')] : []),
  ];
}

const finished = [
  'I’ll read the plan first.',
  '> Read 2 files, ran 1 command',
  'The plan misses one case. Checking it.',
  '> Ran 3 commands',
  'Done. The plan needs one more part.',
];

test('words, steps, words, steps, words read in that order, each run one line that opens to its steps', async () => {
  const screen = await render(<View key="thread" state={rowan(worked(true))} />);
  for (const one of [false, true]) {
    await screen.rerender(<View key={String(one)} state={rowan(worked(true), 'done', one)} />);
    await expect
      .poll(() => [one ? 'a chat of one' : 'in a thread', ...read('rowan')])
      .toEqual([one ? 'a chat of one' : 'in a thread', ...finished]);
    const line = page.getByRole('button', { name: 'Read 2 files, ran 1 command' });
    await line.click();
    const run = (bubbleOf('rowan') as Element).querySelectorAll('.work')[0];
    await expect.poll(() => run.querySelectorAll('li').length).toBe(3);
    await expect.element(screen.getByRole('button', { name: /^Ran a command/ }).first()).toBeVisible();
  }
});

test('when the turn ends nothing moves: the same pieces in the same places, the last run’s line its finished words', async () => {
  const screen = await render(<View state={rowan(worked(false), 'running')} />);
  await expect.poll(() => read('rowan')).toEqual([...finished.slice(0, 3), '> Working']);
  const first = page.getByText('I’ll read the plan first.').element();
  const line = (bubbleOf('rowan') as Element).querySelector('.work') as Element;
  // Within the bubble: the chat itself scrolls to keep its foot as the final words come.
  const placed = () => {
    const top = (bubbleOf('rowan') as Element).getBoundingClientRect().top;
    return [first, line].map((el) => el.getBoundingClientRect().top - top);
  };
  const before = placed();

  await screen.rerender(<View state={rowan(worked(true))} />);
  await expect.poll(() => read('rowan')).toEqual(finished);
  expect(page.getByText('I’ll read the plan first.').element()).toBe(first);
  expect((bubbleOf('rowan') as Element).querySelector('.work')).toBe(line);
  expect(placed()).toEqual(before);
});

test('a run’s line counts what it did by kind: three kinds at most, then "and N more"; a thought reads as itself', () => {
  const thought = (id: string, ms: number): Message => ({
    id,
    turn: 'rowan',
    time: '2026-11-08T09:00:30Z',
    role: 'assistant',
    kind: 'thinking',
    body: { status: 'done', duration_ms: ms },
  });
  const state = rowan([
    thought('t1', 9_000),
    words('One'),
    tool('rowan', 'a1', 'Read'),
    tool('rowan', 'a2', 'Edit'),
    tool('rowan', 'a3', 'Write'),
    tool('rowan', 'a4', 'Bash'),
    tool('rowan', 'a5', 'Grep'),
    tool('rowan', 'a6', 'WebFetch'),
    words('Two'),
    tool('rowan', 'g1', 'mcp__gathr__give_task'),
    tool('rowan', 'g2', 'mcp__gathr__give_task'),
    thought('t2', 4_000),
    words('Three'),
    tool('rowan', 'b1', 'Grep'),
    tool('rowan', 'b2', 'Glob'),
    words('Four'),
  ]);
  state.tasks = [...state.tasks, task(8, 'kit', 'rowan'), task(9, 'lane', 'rowan')];
  const block = toThread(state, agents, [], new Map(), opened).every.find((m) => m.id === 'a-rowan');
  expect(custom(block).pieces?.map((p) => ('run' in p ? p.run.label : p.words))).toEqual([
    'Thought for 9s',
    0,
    'Read 1 file, edited 2 files, ran 1 command and 2 more',
    1,
    'Gave 2 tasks',
    2,
    '2 steps',
    3,
  ]);
});
