// A turn's block in the chat as SerenityOS's card reads it (0082, the owner's "Inside the steps"): in the agent's one
// bubble, its message (the words after its last step), and at the foot one line for every step and every piece of
// narration, in order. Its name line says its title and how long it took. Made-up names and words only.
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message } from '../api';
import { MentionsContext } from '../components/Markdown';
import { agents, alone, custom, opened, people, round, said, task, tool } from './madeUp';
import { toThread } from './messages';
import { noActions } from './recorded';
import type { ChatState } from './state';
import { Thread } from './Thread';

function View({ state }: { state: ChatState }) {
  const cache = useRef(new Map());
  return (
    <MentionsContext value={people}>
      <div style={{ height: 900, width: 900, display: 'flex' }}>
        <Thread
          messages={toThread(state, agents, [], cache.current, opened).items}
          running={false}
          actions={noActions}
          empty={false}
        />
      </div>
    </MentionsContext>
  );
}

const blockOf = (agent: string) => document.querySelector(`.thread-messages .said[data-agent="${agent}"]`);
const bubbleOf = (agent: string) => blockOf(agent)?.querySelector('.bubble');

/** What an agent's bubble reads, piece by piece: its words, and "> " before the steps line's label. */
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
  expect(blockOf('rowan')?.querySelector('.name-line-name')?.textContent).toBe('Rowan');

  await screen.rerender(<View state={rowan([], 'queued')} />);
  await expect.poll(() => bubbleOf('rowan')).toBeFalsy();
});

test('words before a later step are narration in the line at the foot; the message is what came after the last step', async () => {
  const msgs = [
    words('I’ll read the plan first.'),
    tool('rowan', 's1', 'Read'),
    tool('rowan', 's2', 'Bash'),
    words('The plan misses one case. Checking it.'),
    tool('rowan', 's3', 'Bash'),
    words('Done. The plan needs one more part.'),
    // Gathr's calls that only report or place are no step: the words before them stay the message.
    tool('rowan', 's4', 'mcp__gathr__react'),
    tool('rowan', 's5', 'mcp__gathr__finish_task'),
  ];
  for (const one of [false, true]) {
    const screen = await render(<View key={String(one)} state={rowan(msgs, 'done', one)} />);
    await expect
      .poll(() => [one ? 'a chat of one' : 'in a thread', ...read('rowan')])
      .toEqual([
        one ? 'a chat of one' : 'in a thread',
        'Done. The plan needs one more part.',
        '> Read 1 file, ran 2 commands',
      ]);
    // Open, the line lists the narration and each step in the order they came.
    await page.getByRole('button', { name: 'Read 1 file, ran 2 commands' }).click();
    const entries = () =>
      [...(bubbleOf('rowan')?.querySelectorAll('.work li') ?? [])].map((li) =>
        li.querySelector('.work-say')
          ? `"${li.textContent}"`
          : // A Tool call keeps its running label beside its finished one, faded out: the finished one is read.
            (li.textContent ?? '').replace(/^[A-Z][a-z]+ing [^A-Z]*/, '').trim(),
      );
    await expect
      .poll(entries)
      .toEqual([
        '"I’ll read the plan first."',
        'Read a file',
        'Ran a command',
        '"The plan misses one case. Checking it."',
        'Ran a command',
        'Gave a thumbs-up',
        'Finished its task',
      ]);
    expect(bubbleOf('rowan')?.lastElementChild?.matches('.work')).toBe(true);
    await screen.unmount();
  }
});

test('a finished turn that wrote nothing after its last step has its last words as its message', async () => {
  const msgs = [
    words('First, the list.'),
    tool('rowan', 's1', 'Bash'),
    words('The filter ignores tags.'),
    tool('rowan', 's2', 'Read'),
  ];
  await render(<View state={rowan(msgs)} />);
  await expect
    .poll(() => read('rowan'))
    .toEqual(['The filter ignores tags.', '> Ran 1 command, read 1 file']);
  expect(bubbleOf('rowan')?.textContent).not.toContain('First, the list.');
});

test('the steps line counts by kind: three kinds at most, then "and N more"; a thought reads as itself', () => {
  const thought = (id: string, ms: number): Message => ({
    id,
    turn: 'rowan',
    time: '2026-11-08T09:00:30Z',
    role: 'assistant',
    kind: 'thinking',
    body: { status: 'done', duration_ms: ms },
  });
  const line = (msgs: Message[], tasks = 0) => {
    const state = rowan([...msgs, words('Four')]);
    state.tasks = [...state.tasks, ...[task(8, 'kit', 'rowan'), task(9, 'lane', 'rowan')].slice(0, tasks)];
    const block = toThread(state, agents, [], new Map(), opened).every.find((m) => m.id === 'a-rowan');
    return custom(block).line?.label;
  };
  const many = ['Read', 'Edit', 'Write', 'Bash', 'Grep', 'WebFetch'].map((t, i) => tool('rowan', `a${i}`, t));
  expect(line(many)).toBe('Read 1 file, edited 2 files, ran 1 command and 2 more');
  const gave = ['g1', 'g2'].map((id) => tool('rowan', id, 'mcp__gathr__give_task'));
  expect(line(gave, 2)).toBe('Gave 2 tasks');
  expect(line([tool('rowan', 'b1', 'Grep'), tool('rowan', 'b2', 'Glob')])).toBe('2 steps');
  expect(line([thought('t1', 9_000)])).toBe('Thought for 9s');
});

test('the name line says the lead’s and the reviewer’s title, and how long a finished turn took', async () => {
  const state = rowan([words('Cursor fixed')]);
  const made = state.turns.rowan.created as string;
  state.turns.rowan = { ...state.turns.rowan, ended: new Date(Date.parse(made) + 74_000).toISOString() };
  const screen = await render(<View state={state} />);
  const title = (agent: string) => blockOf(agent)?.querySelector('.name-line-title')?.textContent;
  await expect.poll(() => title('mira')).toBe('Lead');
  expect(title('rowan')).toBeUndefined();
  expect(blockOf('rowan')?.querySelector('.name-line-took')?.textContent).toBe('1m 14s');

  // While it runs, the working line has the time: the name line none.
  await screen.rerender(<View state={rowan([words('Cursor fixed')], 'running')} />);
  await expect.poll(() => read('rowan')).toEqual(['Cursor fixed', '> Working']);
  expect(blockOf('rowan')?.querySelector('.name-line-took')).toBeNull();
});

test('an @Name of the chat’s in an agent’s words reads as the blue mention; any other @word stays text', async () => {
  await render(<View state={rowan([words('@Kit can you look at `@Mira`? @Nobody knows.')])} />);
  await expect.poll(() => bubbleOf('rowan')?.querySelectorAll('.mention').length).toBe(1);
  expect(bubbleOf('rowan')?.querySelector('.mention')?.textContent).toBe('@Kit');
});

test('a quote in an agent’s words is upright and muted beside its rule, not Markdown’s italic', async () => {
  await render(<View state={rowan([words('> Mira: the plan holds.\n\nIt misses one case.')])} />);
  await expect.poll(() => bubbleOf('rowan')?.querySelector('blockquote')).toBeTruthy();
  const quote = getComputedStyle(bubbleOf('rowan')?.querySelector('blockquote') as Element);
  expect([quote.fontStyle, quote.borderLeftStyle]).toEqual(['normal', 'solid']);
});
