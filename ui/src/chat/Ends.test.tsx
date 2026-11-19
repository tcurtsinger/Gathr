// How a turn or a task ended: no word on the name line (the owner's call of 2026-11-18), and what the person can do
// under the bubble: Continue, Retry or Sign in.
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Failure, type Message, type Turn } from '../api';
import { agents, opened, round, said, task, tool, turn } from './madeUp';
import { type SendFailure, toThread } from './messages';
import type { ChatState } from './state';
import { type Actions, Blocks, Thread } from './Thread';

const none = () => {};
const quiet: Actions = {
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

function View({
  state,
  unsent = [],
  actions = quiet,
}: {
  state: ChatState;
  unsent?: SendFailure[];
  actions?: Actions;
}) {
  const cache = useRef(new Map());
  return (
    <div style={{ height: 900, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, unsent, cache.current, opened).items}
        running={false}
        actions={actions}
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
const rowan = () => document.querySelector('.said[data-agent="rowan"]');
/** The members' turns as the ask's, not their own tasks'. */
function inTheAsk(state: ChatState): ChatState {
  for (const id of ['rowan', 'kit']) state.turns[id] = { ...state.turns[id], topic: state.turns.ask.topic };
  return state;
}
const button = (name: string) => page.getByRole('button', { name, exact: true });
const ENDS =
  /Gathr restarted|Needs plan|Not needed|Usage limit|Gathr ended it|Failed|Cancelled|Stopped|Not sent/;

/** Rowan's turn on task 1, ended as `more` says, with the words it had said by then. */
function ended(more: Partial<Turn>, words = true): ChatState {
  const state = inTheAsk(round());
  state.tasks = [task(1, 'rowan', 'ask', { state: 'working' }), task(2, 'kit', 'ask')];
  state.turns.rowan = { ...state.turns.rowan, ...more };
  if (!words) state.messages = state.messages.filter((m) => !(m.turn === 'rowan' && m.role === 'assistant'));
  return state;
}

test('a turn a restart cut offers Continue, with no word for it and never as failed', async () => {
  const state = ended({ status: 'stopped', end: 'restart', why: 'the service stopped during this turn' });
  await render(<View state={state} />);
  await expect.element(button('Continue')).toBeVisible();
  expect(lines()).toContain('Rowan');
  expect(document.body.textContent).not.toMatch(ENDS);
});

test('a turn Gathr ended offers nothing, so with no words it draws nothing: it does not read as the person’s stop', async () => {
  const state = inTheAsk(round());
  // Kit's queued task held for its plan, and Rowan's plan check no longer needed.
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
  await render(<View state={state} />);
  await expect.element(page.getByText('Rowan and Kit, over to you')).toBeVisible();
  expect(lines().filter((l) => /^(Rowan|Kit)/.test(l))).toEqual([]);
  expect(button('Continue').elements()).toHaveLength(0);
  expect(document.body.textContent).not.toMatch(ENDS);
});

test('the person’s stop is the face’s mark, with no word: Continue stays under it until its carry-on runs', async () => {
  const state = ended({ status: 'stopped', end: 'you' });
  const carryOn: string[] = [];
  const actions: Actions = { ...quiet, carryOn: (agent) => carryOn.push(agent ?? '') };
  const screen = await render(<View state={state} actions={actions} />);
  expect(lines()).toContain('Rowan');
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

test('a failed turn has its reference and Retry under the bubble, and no word on its name line', async () => {
  const retried: string[] = [];
  const opened: string[] = [];
  const actions: Actions = {
    ...quiet,
    diagnostics: (t) => opened.push(t ?? ''),
    retry: (t) => retried.push(t),
  };
  const state = ended({ status: 'failed', why: 'the engine exited', ref: '5236079d' });
  await render(<View state={state} actions={actions} />);
  await button('Reference 5236079d').click();
  await button('Retry').click();
  expect([opened, retried]).toEqual([['rowan'], ['rowan']]);
  expect(lines()).toContain('Rowan');
  expect(document.body.textContent).not.toMatch(/Failed|Reply failed|Send failed/);
});

test('a message that was not sent says why under it, with its reference and Retry, and no word on its name line', async () => {
  const resent: SendFailure[] = [];
  const failure = new Failure('Send failed', 'The service isn’t running.', 'a0c5a2a6');
  const unsent = [{ id: '1', text: 'Ship it once the review agrees.', failure }];
  await render(
    <View state={round()} unsent={unsent} actions={{ ...quiet, resend: (f) => resent.push(f) }} />,
  );
  await expect.element(page.getByText('The service isn’t running.', { exact: true })).toBeVisible();
  expect(lines().at(-1)).toBe('You');
  await expect.element(button('Reference a0c5a2a6')).toBeVisible();
  await button('Retry').click();
  expect(resent.map((f) => f.text)).toEqual(['Ship it once the review agrees.']);
  expect(document.body.textContent).not.toMatch(/Not sent/);
});

test('a turn waiting for a sign-in offers Sign in; one held for its engine’s download draws nothing', async () => {
  const screen = await render(<View state={ended({ status: 'waiting_sign_in' }, false)} />);
  await expect.element(button('Sign in')).toBeVisible();
  expect(lines()).toContain('Rowan');
  expect(document.body.textContent).not.toMatch(/Waiting for sign-in/);
  await screen.rerender(<View state={ended({ status: 'queued', held: 'claude' }, false)} />);
  await expect.element(button('Sign in')).not.toBeInTheDocument();
  expect(rowan()).toBeNull();
  expect(document.body.textContent).not.toMatch(/downloading/);
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

test('how a task ended is no tick and no word in the chat, live or after a reload, and no line says it', async () => {
  const screen = await render(<View state={twice('cancelled')} />);
  await expect.element(page.getByText('Fixed that too')).toBeVisible();
  expect(lines().filter((l) => l.startsWith('Rowan'))).toEqual(['Rowan', 'Rowan']);
  expect(document.body.textContent).not.toMatch(ENDS);
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
  await screen.rerender(<View state={live} />);
  await expect.element(page.getByText('Rowan is done; Kit carries on')).toBeVisible();
  const ticks = () => document.querySelectorAll('.name-line svg').length;
  expect(ticks()).toBe(0);
  expect(document.body.textContent).not.toMatch(/completed|A task is done/);
  // After a reload the history holds the same messages, Gathr's line placed where it was said.
  const reloaded = {
    ...live,
    messages: [...live.messages.slice(0, -3), heard, line, live.messages.at(-1) as Message],
  };
  await screen.rerender(<View state={reloaded} />);
  expect(ticks()).toBe(0);
  expect(document.body.textContent).not.toMatch(/completed|A task is done/);
});

test('a refused step reads Refused, and its run never counts it as a task given', async () => {
  const state = round();
  state.messages.push(
    tool('ask', 'give', 'mcp__gathr__give_task', { status: 'error', input: { text: 'Bo: Tag picker' } }),
    said('ask', 'assistant', 'Tags wait for the plan'),
  );
  const mira = toThread(state, agents, [], new Map(), opened).every.filter((b) => b.id === 'a-ask');
  await render(<Blocks messages={mira} actions={quiet} />);
  const steps = page.getByRole('button', { name: '1 step', exact: true });
  await expect.element(steps).toBeInTheDocument();
  (steps.element() as HTMLButtonElement).focus();
  await userEvent.keyboard('{Enter}');
  await expect.element(page.getByText('Refused', { exact: true })).toBeVisible();
  expect(document.body.textContent).not.toMatch(/Gave a task/);
});
