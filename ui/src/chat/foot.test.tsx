// What opens in place at the very foot of the chat shows above the message box, measured as the window lays it out.
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { useFolds } from './folds';
import { agents, at, members, said, task, turn } from './madeUp';
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

/** The chat at 700 tall; threads handled when it opens start folded, as in the chat. */
function View({ state }: { state: ChatState }) {
  const cache = useRef(new Map());
  const threads = useFolds();
  const timeline = toThread(state, agents, [], cache.current, {
    ...threads.folds,
    start: threads.start.current,
  });
  threads.settle(timeline, true);
  return (
    <div style={{ height: 700, display: 'flex' }}>
      <Thread
        messages={timeline.items}
        running={false}
        actions={{ ...actions, fold: threads.fold }}
        empty={false}
      >
        <div style={{ height: 120 }}>Message box</div>
      </Thread>
    </div>
  );
}

/**
 * Twenty earlier exchanges, then Rowan's task: its PR, reviewed by Sol and fixed by Rowan, all settled. Its thread is
 * folded, at the foot of the chat.
 */
function atTheFoot(): ChatState {
  // What each turn is about, as the service names it (0075): each exchange its question; the work on the PR, task 1.
  const earlier = Array.from({ length: 20 }, (_, i) => turn(`t${i}`, 'mira', { topic: `m:q${i}` }));
  const turns = [
    ...earlier,
    turn('ask', 'mira', { topic: 'm:ask' }),
    turn('build', 'rowan', { task: 1, answers: 'ask', topic: 't:1' }),
    turn('review', 'sol', { task: 2, topic: 't:1' }),
    turn('fix', 'rowan', { task: 3, topic: 't:1' }),
  ];
  return {
    ...empty,
    members,
    turns: Object.fromEntries(turns.map((t, position) => [t.id, { ...t, position }])),
    messages: [
      ...earlier.flatMap((t, i) => [
        { ...said(t.id, 'user', `Made-up question ${i}`), id: `q${i}` },
        said(t.id, 'assistant', `Made-up answer ${i}`),
      ]),
      { ...said('ask', 'user', 'Parse recipe pages'), id: 'ask' },
      said('ask', 'assistant', 'Rowan, over to you'),
      said('build', 'gathr', 'Task #1 from Mira: Parse recipe pages'),
      said('build', 'assistant', 'The parser is in. PR #18 is open'),
      said('review', 'gathr', 'Task #2 from Mira: Review PR #18'),
      said('review', 'assistant', 'One problem: redirects reach private addresses'),
      said('fix', 'gathr', 'Task #3 from Sol: Fix the review of #18'),
      said('fix', 'assistant', 'Fixed, with a test for each'),
    ],
    tasks: [
      task(1, 'rowan', 'ask', {
        what: 'Parse recipe pages',
        pr: 18,
        state: 'done',
        created_us: Date.parse(at()) * 1000,
      }),
      task(2, 'sol', 'build', { kind: 'review', what: 'Review PR #18', pr: 18, state: 'done' }),
      task(3, 'rowan', 'review', {
        giver: 'sol',
        what: 'Fix the review of #18',
        pr: 18,
        by_gathr: true,
        state: 'done',
      }),
    ],
  };
}

/** How far below the message box's top edge an element's bottom sits. */
function belowTheBox(el: Element) {
  const footer = document.querySelector('.thread-footer') as HTMLElement;
  return Math.round(el.getBoundingClientRect().bottom - footer.getBoundingClientRect().top);
}

async function atBottom() {
  const viewport = document.querySelector('.thread-viewport') as HTMLElement;
  await expect.poll(() => viewport.scrollHeight > viewport.clientHeight).toBe(true);
  await userEvent.wheel(viewport, { delta: { y: 100_000 } });
  await expect.poll(() => viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop).toBeLessThan(2);
  // Off the rows, so none shows its hover.
  await userEvent.unhover(viewport);
}

test('a thread opened at the foot of the chat scrolls up to show above the message box', async () => {
  const screen = await render(<View state={atTheFoot()} />);
  await atBottom();
  await screen.getByRole('button', { name: /3 replies/ }).click();
  const last = screen.getByText('Fixed, with a test for each');
  await expect.element(last).toBeVisible();
  await expect.poll(() => belowTheBox(last.element())).toBeLessThanOrEqual(0);
});
