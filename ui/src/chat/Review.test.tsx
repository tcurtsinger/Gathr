// A review's turn, and the fix that came of it, read in the thread of the work task whose PR it reviews (0072, 0075):
// each with its own name line, open while anything in it is open, and folded when the chat is opened again once all
// of it is settled; never by itself while the person looks.
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { TaskState } from '../api';
import { blocksOf, noneKept } from './AgentPanel';
import { activity } from './glance';
import { agents, members, opened, said, task, turn, view } from './madeUp';
import { toThread } from './messages';
import { View as Chat, noActions } from './recorded';
import { type ChatState, empty } from './state';
import { Thread } from './Thread';

function View({ state }: { state: ChatState }) {
  const cache = useRef(new Map());
  return (
    <div style={{ height: 900, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, [], cache.current, opened).items}
        running={false}
        actions={noActions}
        empty={false}
      />
    </div>
  );
}

/**
 * Rowan builds task 1 and opens PR #18; the person asks Mira something else; Sol reviews the PR (task 2) and Rowan
 * fixes what the review found (task 3, Gathr's fix on the same PR). `review` and `fix` are those tasks' states. Each
 * turn is about what the service says it is: the review and the fix about task 1, whose PR they are on.
 */
function reviewed(review: TaskState, fix: TaskState): ChatState {
  const messages = [
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
  ];
  const turns = [
    turn('ask', 'mira', { topic: `m:${messages[0].id}` }),
    turn('build', 'rowan', { task: 1, answers: 'ask', topic: 't:1' }),
    turn('later', 'mira', { topic: `m:${messages[4].id}` }),
    turn('review', 'sol', { task: 2, topic: 't:1' }),
    turn('fix', 'rowan', { task: 3, topic: 't:1' }),
  ];
  const given = Date.parse(messages[1].time) * 1000;
  return {
    ...empty,
    members,
    turns: Object.fromEntries(turns.map((t, position) => [t.id, { ...t, position }])),
    messages,
    tasks: [
      task(1, 'rowan', 'ask', { what: 'Parse recipe pages', pr: 18, created_us: given, state: 'held' }),
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

test("a review and its fix read in the work task's thread, after its build, never on the main line", async () => {
  await render(<View state={reviewed('blocked', 'working')} />);
  await expect.element(page.getByText('Fixed, with a test for each', { exact: true })).toBeVisible();
  // In the thread of the task they are about, before what the person said after it was handed out.
  expect(
    order(
      'Rowan, over to you',
      'The parser is in. PR #18 is open',
      'One problem: redirects reach private addresses',
      'Fixed, with a test for each',
      'And the dark theme?',
    ),
  ).toBe(true);
  // Each keeps its own name line, who said it; the task it worked on is its agent's panel's.
  const named = [...document.querySelectorAll('.said[data-reply] .name-line-name')].map((l) => l.textContent);
  expect(named).toEqual(expect.arrayContaining(['Sol', 'Rowan']));
  const review = page.getByText('One problem: redirects reach private addresses').element();
  expect(review.closest('.said')?.hasAttribute('data-reply')).toBe(true);
});

test('once settled it stays as it is while the chat is open, and starts folded when the chat is opened again', async () => {
  const screen = await render(<Chat state={reviewed('blocked', 'working')} />);
  await expect.element(page.getByText('Fixed, with a test for each', { exact: true })).toBeVisible();
  // Settled live: it never folds under the person's eyes.
  const done = reviewed('done', 'done');
  done.tasks[0] = { ...done.tasks[0], state: 'done' };
  await screen.rerender(<Chat state={done} />);
  await expect.element(page.getByText('Fixed, with a test for each', { exact: true })).toBeVisible();

  // Opened again: settled already, it starts folded, and a press opens it.
  await screen.unmount();
  await render(<Chat state={done} />);
  const shut = page.getByRole('button', { name: /3 replies/ });
  await expect.element(shut).toBeVisible();
  expect(shut.element().getAttribute('aria-expanded')).toBe('false');
  await expect
    .element(page.getByText('Fixed, with a test for each', { exact: true }))
    .not.toBeInTheDocument();
  await shut.click();
  await expect.element(page.getByText('Fixed, with a test for each', { exact: true })).toBeVisible();
});

test('in a folded thread, a review is still its reviewer’s: on its panel, and as its running row', () => {
  const done = view(reviewed('done', 'done')).every;
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
  const all = view(running).every;
  expect(blocksOf(all, 'sol', noneKept()).now?.running).toBe(true);
  expect(activity(all).has('sol')).toBe(true);
});
