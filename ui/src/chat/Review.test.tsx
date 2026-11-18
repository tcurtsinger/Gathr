// A review's turn reads under the pull request it reviews (0072): a thread under the builder's message, with the fix
// that came of it, open while anything in it is open and folded once all of it is settled.
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { TaskState } from '../api';
import { blocksOf, noneKept } from './AgentPanel';
import { activity } from './glance';
import { agents, members, said, task, turn, view } from './madeUp';
import { toThread } from './messages';
import { type ChatState, empty } from './state';
import { type Actions, Thread } from './Thread';

const none = () => {};
const actions: Actions = {
  openTask: none,
  openAgent: none,
  diagnostics: none,
  reference: none,
  signIn: none,
  retry: none,
  resend: none,
  carryOn: none,
  approve: none,
};

function View({ state }: { state: ChatState }) {
  const cache = useRef(new Map());
  return (
    <div style={{ height: 900, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, [], cache.current).items}
        running={false}
        actions={actions}
        empty={false}
      />
    </div>
  );
}

/**
 * Rowan builds task 1 and opens PR #18; the person asks Mira something else; Sol reviews the PR (task 2) and Rowan
 * fixes what the review found (task 3, Gathr's fix on the same PR). `review` and `fix` are those tasks' states.
 */
function reviewed(review: TaskState, fix: TaskState): ChatState {
  const turns = [
    turn('ask', 'mira'),
    turn('build', 'rowan', { task: 1, answers: 'ask' }),
    turn('later', 'mira'),
    turn('review', 'sol', { task: 2 }),
    turn('fix', 'rowan', { task: 3 }),
  ];
  return {
    ...empty,
    members,
    turns: Object.fromEntries(turns.map((t, position) => [t.id, { ...t, position }])),
    messages: [
      said('ask', 'user', 'Parse recipe pages'),
      said('ask', 'assistant', 'Rowan, over to you'),
      said('build', 'gathr', 'Task #1 from Mira: Parse recipe pages'),
      said('build', 'assistant', 'The parser is in. PR #18 is open'),
      said('later', 'user', 'And the dark theme?'),
      said('later', 'assistant', 'Next, once the parser lands'),
      said('review', 'gathr', 'Task #2 from Mira: Review PR #18'),
      said('review', 'assistant', 'One problem: redirects reach private addresses'),
      said('fix', 'gathr', 'Task #3 from Sol: Fix the review of #18'),
      said('fix', 'assistant', 'Fixed, with a test for each'),
    ],
    tasks: [
      task(1, 'rowan', 'ask', { what: 'Parse recipe pages', pr: 18 }),
      task(2, 'sol', 'build', { kind: 'review', what: 'Review PR #18', pr: 18, state: review }),
      task(3, 'rowan', 'review', {
        giver: 'sol',
        what: 'Fix the review of #18',
        pr: 18,
        by_gathr: true,
        state: fix,
      }),
    ],
  };
}

/** The drawn messages, top to bottom, by their words. */
const order = (...words: string[]) =>
  words
    .map((w) => page.getByText(w, { exact: true }).element())
    .every(
      (el, i, all) => i === 0 || all[i - 1].compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING,
    );

test('a review and its fix read under the PR’s message, open while the review blocks', async () => {
  const screen = await render(<View state={reviewed('blocked', 'working')} />);
  const thread = screen.getByRole('region', { name: 'Replies' });
  await expect.element(thread).toBeVisible();
  expect(thread.element().textContent).toContain('One problem: redirects reach private addresses');
  expect(thread.element().textContent).toContain('Fixed, with a test for each');
  // Under the builder's message, before what the person said in between.
  expect(
    order(
      'The parser is in. PR #18 is open',
      'One problem: redirects reach private addresses',
      'Fixed, with a test for each',
      'And the dark theme?',
    ),
  ).toBe(true);
  // Each keeps its own name line: who, and the task it worked on.
  const named = [...thread.element().querySelectorAll('.name-line')].map((l) => l.textContent);
  expect(named).toHaveLength(2);
  expect(named[0]).toContain('Sol');
  expect(named[0]).toContain('on Review PR #18');
  expect(named[1]).toContain('Rowan');
  expect(named[1]).toContain('on Fix the review of #18');
  await expect.element(screen.getByRole('button', { name: 'Hide' })).toBeVisible();
});

test('once the review and its fix are settled the thread folds, and opens again on a click', async () => {
  const screen = await render(<View state={reviewed('blocked', 'working')} />);
  await expect.element(page.getByText('Fixed, with a test for each', { exact: true })).toBeVisible();
  // Settled live: it folds by itself.
  await screen.rerender(<View state={reviewed('done', 'done')} />);
  const shut = screen.getByRole('button', { name: '2 replies' });
  await expect.element(shut).toBeVisible();
  await expect
    .element(page.getByText('Fixed, with a test for each', { exact: true }))
    .not.toBeInTheDocument();
  await shut.click();
  await expect.element(page.getByText('Fixed, with a test for each', { exact: true })).toBeVisible();

  // Opened after a reload: settled already, it starts folded.
  await screen.unmount();
  const again = await render(<View state={reviewed('done', 'done')} />);
  await expect.element(again.getByRole('button', { name: '2 replies' })).toBeVisible();
});

test('in a thread under its PR, a review is still its reviewer’s: on its panel, and as its running row', () => {
  const done = view(reviewed('done', 'done')).items;
  const sols = blocksOf(done, 'sol', noneKept()).earlier;
  expect(sols.flatMap((b) => ((b.content ?? []) as readonly { text?: string }[]).map((p) => p.text))).toEqual(
    ['One problem: redirects reach private addresses'],
  );

  const running = reviewed('working', 'ready');
  running.turns.review = { ...running.turns.review, status: 'running' };
  running.messages.push({
    ...said('review', 'assistant', ''),
    kind: 'tool',
    body: { tool: 'Read', status: 'running' },
  });
  const items = view(running).items;
  expect(blocksOf(items, 'sol', noneKept()).now?.running).toBe(true);
  expect(activity(items).has('sol')).toBe(true);
});
