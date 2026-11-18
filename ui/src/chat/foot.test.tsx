// What opens in place at the very foot of the chat shows above the message box, measured as the window lays it out.
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { useFolds } from './folds';
import { agents, at, members, opened, said, task, tool, turn } from './madeUp';
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

/** The chat at 700 tall; with `folded`, threads handled when it opens start folded, as in the chat. */
function View({ state, folded }: { state: ChatState; folded?: boolean }) {
  const cache = useRef(new Map());
  const threads = useFolds();
  const view = folded ? { ...threads.folds, start: threads.start.current } : opened;
  const timeline = toThread(state, agents, [], cache.current, view);
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
 * folded, at the foot of the chat. Rowan's PR message took `steps` steps.
 */
function atTheFoot(steps = 3): ChatState {
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
      ...Array.from({ length: steps }, (_, i) =>
        tool('build', `b${i}`, 'Read', { input: { text: `src/part${i}.rs` } }),
      ),
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

/** How far below the message box's top edge, and its fade over the chat while it shows, an element's bottom sits. */
function belowTheBox(el: Element) {
  const footer = document.querySelector('.thread-footer') as HTMLElement;
  const fades = document.querySelector('.thread-viewport')?.hasAttribute('data-fade-below');
  const fade = fades ? Number.parseFloat(getComputedStyle(footer, '::before').height) : 0;
  return Math.round(el.getBoundingClientRect().bottom - (footer.getBoundingClientRect().top - fade));
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
  const screen = await render(<View state={atTheFoot()} folded />);
  await atBottom();
  await screen.getByRole('button', { name: /3 replies/ }).click();
  const last = screen.getByText('Fixed, with a test for each');
  await expect.element(last).toBeVisible();
  await expect.poll(() => belowTheBox(last.element())).toBeLessThanOrEqual(0);
});

test("a turn's steps opened at the foot of the chat scroll up to show above the message box", async () => {
  const screen = await render(<View state={atTheFoot()} />);
  await atBottom();
  const steps = screen.getByRole('button', { name: /^3 steps/ });
  (steps.element() as HTMLElement).focus();
  await userEvent.keyboard('{Enter}');
  const last = () => [...document.querySelectorAll('.steps-only')].at(-1) as Element;
  await expect.poll(() => !!last()).toBe(true);
  await expect.poll(() => belowTheBox(last())).toBeLessThanOrEqual(0);
});

test('steps taller than the chat above the message box keep their button in view as they open', async () => {
  const screen = await render(<View state={atTheFoot(40)} />);
  await atBottom();
  const steps = screen.getByRole('button', { name: /^40 steps/ });
  (steps.element() as HTMLElement).focus();
  await userEvent.keyboard('{Enter}');
  const viewport = document.querySelector('.thread-viewport') as HTMLElement;
  const top = () =>
    Math.round(steps.element().getBoundingClientRect().top - viewport.getBoundingClientRect().top);
  // It moved up from the foot, and stopped clear of the top fade.
  await expect.poll(top).toBeLessThan(300);
  await expect.poll(top).toBeGreaterThanOrEqual(32);
});
