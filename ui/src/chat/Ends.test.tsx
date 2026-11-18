// How a turn or a task ended, on its message's name line in a word, with what the person can do under the bubble:
// Continue, Retry or Sign in.
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Failure, type Message, type Turn } from '../api';
import { agents, opened, round, said, task, tool, turn } from './madeUp';
import { type SendFailure, toThread } from './messages';
import type { ChatState } from './state';
import { type Actions, Thread } from './Thread';

function View({
  state,
  unsent = [],
  actions,
}: {
  state: ChatState;
  unsent?: SendFailure[];
  actions?: Actions;
}) {
  const cache = useRef(new Map());
  const none = () => {};
  const all: Actions = actions ?? {
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
  return (
    <div style={{ height: 900, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, unsent, cache.current, opened).items}
        running={false}
        actions={all}
        empty={false}
      />
    </div>
  );
}

/** Each drawn name line's words but its time, in order, without the "·" a screen reader leaves out too. */
const lines = () =>
  [...document.querySelectorAll('.name-line')].map((l) =>
    [...l.querySelectorAll('.name-line-name, .name-line-meta > :not(time)')]
      .map((p) => p.textContent)
      .join(' | '),
  );
/** The members' turns as the ask's, not their own tasks'. */
function inTheAsk(state: ChatState): ChatState {
  for (const id of ['rowan', 'kit']) state.turns[id] = { ...state.turns[id], topic: state.turns.ask.topic };
  return state;
}
const button = (name: string) => page.getByRole('button', { name, exact: true });
/** The danger colour as the page computes it. */
function danger() {
  const probe = document.createElement('span');
  probe.style.color = 'var(--destructive)';
  document.body.append(probe);
  const color = getComputedStyle(probe).color;
  probe.remove();
  return color;
}

/**
 * Rowan's turn on task 1, ended as `more` says, with the words it had said by then. The members' turns read in the
 * ask's thread here, where a member's message names its task, as it does in any thread but its task's own (0075).
 */
function ended(more: Partial<Turn>, words = true): ChatState {
  const state = inTheAsk(round());
  state.tasks = [task(1, 'rowan', 'ask', { state: 'working' }), task(2, 'kit', 'ask')];
  state.turns.rowan = { ...state.turns.rowan, ...more };
  if (!words) state.messages = state.messages.filter((m) => !(m.turn === 'rowan' && m.role === 'assistant'));
  return state;
}

test('a turn a restart cut reads "Gathr restarted" on its name line, with Continue, and never as failed', async () => {
  const state = ended({ status: 'stopped', end: 'restart', why: 'the service stopped during this turn' });
  const screen = await render(<View state={state} />);
  await expect.element(screen.getByText('Gathr restarted', { exact: true })).toBeVisible();
  expect(lines()).toContain('Rowan | on Task 1 | Gathr restarted');
  await expect.element(button('Continue')).toBeVisible();
  expect(document.body.textContent).not.toMatch(/failed|Stopped/i);
});

test('a turn Gathr ended says why in a word and offers no Continue: it does not read as the person’s stop', async () => {
  const state = inTheAsk(round());
  // Rev's check of a plan the person approved meanwhile, and Kit's queued task held for its plan.
  state.turns.kit = {
    ...state.turns.kit,
    status: 'stopped',
    end: 'plan',
    why: 'its task #2 waits for its plan',
  };
  state.turns.rowan = { ...state.turns.rowan, status: 'stopped', end: 'cancelled' };
  state.tasks = [
    task(1, 'rowan', 'ask', { state: 'cancelled', checks: 4 }),
    task(2, 'kit', 'ask', { state: 'waiting' }),
  ];
  state.messages = state.messages.filter(
    (m) => m.role !== 'assistant' || !['rowan', 'kit'].includes(m.turn ?? ''),
  );
  const screen = await render(<View state={state} />);
  await expect.element(screen.getByText('Needs plan', { exact: true })).toBeVisible();
  expect(lines().filter((l) => /^(Rowan|Kit)/.test(l))).toEqual([
    'Rowan | on Task 1 | Not needed',
    'Kit | on Task 2 | Needs plan',
  ]);
  expect(button('Continue').elements()).toHaveLength(0);
  // The turns' own words: the task's message says the task was cancelled, at its right (0075).
  expect(lines().join(' ')).not.toMatch(/Stopped|Cancelled/);
});

test('the person’s stop is the face’s mark, with no word: Continue stays under it until its carry-on runs', async () => {
  const state = ended({ status: 'stopped', end: 'you' });
  const carryOn: string[] = [];
  const none = () => {};
  const actions: Actions = {
    openTask: none,
    openAgent: none,
    diagnostics: none,
    reference: none,
    signIn: none,
    retry: none,
    resend: none,
    carryOn: (agent) => {
      carryOn.push(agent ?? '');
    },
    approve: none,
  };
  const screen = await render(<View state={state} actions={actions} />);
  expect(lines()).toContain('Rowan | on Task 1');
  await button('Continue').click();
  // A member's Continue goes to that member, not to the lead an unnamed message goes to.
  expect(carryOn).toEqual(['rowan']);
  state.turns.again = {
    ...turn('again', 'rowan', { task: 1, answers: 'rowan', status: 'running' }),
    position: 9,
  };
  await screen.rerender(<View state={{ ...state }} actions={actions} />);
  await expect.element(button('Continue')).not.toBeInTheDocument();
  expect(document.body.textContent).not.toMatch(/carried on|Stopped/);
});

test('a failed turn says Failed on its name line, with its reference and Retry under the bubble', async () => {
  const retried: string[] = [];
  const opened: string[] = [];
  const none = () => {};
  const actions: Actions = {
    openTask: none,
    openAgent: none,
    diagnostics: (t) => opened.push(t ?? ''),
    reference: none,
    signIn: none,
    retry: (t) => retried.push(t),
    resend: none,
    carryOn: none,
    approve: none,
  };
  const state = ended({ status: 'failed', why: 'the engine exited', ref: '5236079d' });
  await render(<View state={state} actions={actions} />);
  const word = page.getByText('Failed', { exact: true });
  await expect.element(word).toBeVisible();
  expect(lines()).toContain('Rowan | on Task 1 | Failed');
  expect(getComputedStyle(word.element()).color).toBe(danger());
  await button('Reference 5236079d').click();
  await button('Retry').click();
  expect([opened, retried]).toEqual([['rowan'], ['rowan']]);
  expect(document.body.textContent).not.toMatch(/Reply failed|Send failed/);
});

test('a message that was not sent says so on its name line, with why, its reference and Retry', async () => {
  const resent: SendFailure[] = [];
  const none = () => {};
  const actions: Actions = {
    openTask: none,
    openAgent: none,
    diagnostics: none,
    reference: none,
    signIn: none,
    retry: none,
    resend: (f) => resent.push(f),
    carryOn: none,
    approve: none,
  };
  const failure = new Failure('Send failed', 'The service isn’t running.', 'a0c5a2a6');
  const unsent = [{ id: '1', text: 'Ship it once the review agrees.', failure }];
  await render(<View state={round()} unsent={unsent} actions={actions} />);
  await expect.element(page.getByText('Not sent', { exact: true })).toBeVisible();
  expect(lines().at(-1)).toBe('You | Not sent');
  await expect.element(page.getByText('The service isn’t running.', { exact: true })).toBeVisible();
  await expect.element(button('Reference a0c5a2a6')).toBeVisible();
  await button('Retry').click();
  expect(resent.map((f) => f.text)).toEqual(['Ship it once the review agrees.']);
});

test('a turn waiting for a sign-in offers Sign in; one held for its engine’s download draws nothing', async () => {
  const screen = await render(<View state={ended({ status: 'waiting_sign_in' }, false)} />);
  await expect.element(button('Sign in')).toBeVisible();
  expect(document.body.textContent).not.toMatch(/Waiting for sign-in/);
  await screen.rerender(<View state={ended({ status: 'queued', held: 'claude' }, false)} />);
  await expect.element(button('Sign in')).not.toBeInTheDocument();
  expect(document.body.textContent).not.toMatch(/downloading/);
});

test('a task cancelled under its member’s turn says Cancelled on that turn’s message, though it has no words', async () => {
  const state = ended(
    { status: 'stopped', end: 'cancelled', why: 'its task #1 was cancelled: the PR was closed' },
    false,
  );
  state.tasks[0] = { ...state.tasks[0], state: 'cancelled' };
  await render(<View state={state} />);
  await expect.poll(lines).toContain('Rowan | on Task 1 | Cancelled');
  expect(button('Continue').elements()).toHaveLength(0);
});

/** Rowan works on task 1 in two turns with the lead's words between them; the task ends `state`. */
function twice(state: string): ChatState {
  const chat = inTheAsk(round());
  chat.tasks = [
    task(1, 'rowan', 'ask', { state: state as never }),
    task(2, 'kit', 'ask', { state: 'working' }),
  ];
  chat.turns.fixup = {
    ...turn('fixup', 'rowan', { task: 1, answers: 'mid', topic: chat.turns.ask.topic }),
    position: 9,
  };
  chat.messages.push(
    said('mid', 'assistant', 'Rowan, one more thing'),
    said('fixup', 'assistant', 'Fixed that too'),
  );
  return chat;
}
const ticks = () => document.querySelectorAll('.name-line [aria-label="Done"]').length;

test('how a task ended sits on its member’s last message: a tick for done, Cancelled and Failed as words', async () => {
  const screen = await render(<View state={twice('done')} />);
  await expect.element(page.getByText('Fixed that too')).toBeVisible();
  expect(ticks()).toBe(1);
  const rowans = document.querySelectorAll('.said[data-agent="rowan"]');
  expect(rowans[rowans.length - 1].querySelector('.name-line [aria-label="Done"]')).not.toBeNull();
  expect(lines().filter((l) => l.startsWith('Rowan'))).toEqual(['Rowan | on Task 1', 'Rowan | on Task 1']);
  for (const [state, word] of [
    ['cancelled', 'Cancelled'],
    ['failed', 'Failed'],
  ]) {
    await screen.rerender(<View state={twice(state)} />);
    // Once on the turns, on the last: the task's own message says it too, as its state (0075).
    await expect.poll(() => lines().filter((l) => l.endsWith(word))).toHaveLength(1);
    expect(
      lines()
        .filter((l) => l.startsWith('Rowan'))
        .at(-1),
    ).toBe(`Rowan | on Task 1 | ${word}`);
  }
  // Blocked, stopped and held to merge are the face's mark: no word, and never a tick.
  for (const state of ['blocked', 'stopped', 'held']) {
    await screen.rerender(<View state={twice(state)} />);
    expect(ticks()).toBe(0);
    expect(
      lines()
        .filter((l) => l.startsWith('Rowan'))
        .at(-1),
    ).toBe('Rowan | on Task 1');
  }
});

test('a task that ends done shows its tick once, live and after a reload, and no line says it again', async () => {
  // Live: Gathr's line for the done task comes as it ends, then the lead's hear-back turn opens on the round.
  const live = twice('done');
  const line: Message = {
    ...said('none', 'gathr', 'A task is done:\n- #1 Rowan, Task 1: done: Cursor fixed'),
    turn: null,
  };
  live.turns.back = { ...turn('back', 'mira', { answers: 'ask' }), position: 10 };
  const heard = said(
    'back',
    'gathr',
    'The task you gave is settled:\n- #1 Rowan, Task 1: done: Cursor fixed\nRead.',
  );
  live.messages.push(line, heard, said('back', 'assistant', 'Rowan is done; Kit carries on'));
  const screen = await render(<View state={live} />);
  await expect.element(page.getByText('Rowan is done; Kit carries on')).toBeVisible();
  expect(ticks()).toBe(1);
  expect(document.body.textContent).not.toMatch(/completed|A task is done/);
  // After a reload the history holds the same messages, Gathr's line placed where it was said.
  const reloaded = {
    ...live,
    messages: [...live.messages.slice(0, -3), heard, line, live.messages.at(-1) as Message],
  };
  await screen.rerender(<View state={reloaded} />);
  expect(ticks()).toBe(1);
  expect(document.body.textContent).not.toMatch(/completed|A task is done/);
});

test('a refused step reads Refused, and the steps on the name line count it as refused', async () => {
  const state = round();
  state.messages.push(
    tool('ask', 'give', 'mcp__gathr__give_task', { status: 'error', input: { text: 'Bo: Tag picker' } }),
    said('ask', 'assistant', 'Tags wait for the plan'),
  );
  await render(<View state={state} />);
  const steps = page.getByRole('button', { name: /refused/ });
  await expect.element(steps).toBeInTheDocument();
  expect(steps.element().textContent).toMatch(/1 refused/);
  expect(steps.element().textContent).not.toMatch(/failed/);
  // By keyboard: the pointer on a short bubble raises its like row over the steps (found, not this test's subject).
  (steps.element() as HTMLButtonElement).focus();
  await userEvent.keyboard('{Enter}');
  await expect.element(page.getByText('Refused', { exact: true })).toBeVisible();
  expect(document.body.textContent).not.toMatch(/Gave a task/);
});
