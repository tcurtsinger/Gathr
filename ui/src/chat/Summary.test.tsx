// Summary as the person uses it (0038): its button in the chat header, and the popover's plan and team.
import { useState } from 'react';
import { expect, test } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Member, Message, Task, Turn, Wait, Waiting } from '../api';
import { activity, glances } from './glance';
import { toThread } from './messages';
import { Summary } from './Summary';
import { type ChatState, empty } from './state';

const names = ['Made-up Lead', 'Made-up Member', 'Made-up Third', 'Kit', 'Lane'];
const members: Member[] = names.map((name, i) => ({
  agent: 'abcde'[i],
  name,
  role: i === 0 ? 'supervisor' : 'member',
}));
const trio = members.slice(0, 3);
const turn = (id: string, agent: string, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'running',
  why: null,
  ref: id,
  ...more,
});
const task = (number: number, owner: string | null, more: Partial<Task> = {}): Task => ({
  number,
  chat: 'c',
  giver: 'a',
  owner,
  given_in: 'lead',
  kind: 'work',
  what: `Made-up task ${number}`,
  state: 'working',
  reason: null,
  after: [],
  created_us: 0,
  ...more,
});
const tool = (id: string, turnId: string, name: string): Message => ({
  id,
  turn: turnId,
  time: '2026-11-08T09:00:00Z',
  role: 'assistant',
  kind: 'tool',
  body: { tool: name, status: 'running' },
});
const wait = (agent: string, target: string): Wait => ({
  id: `w-${agent}`,
  agent,
  task: null,
  kind: 'agent',
  target,
  state: 'open',
  created_ts: 0,
});
const turns = (...list: Turn[]) => Object.fromEntries(list.map((t) => [t.id, t]));
const approval = (turnId: string) => ({ turn: turnId, request: `q-${turnId}`, tool: 'Bash' });

/** Summary as the chat draws it, with what it reports back kept for the test to read. */
function Shown({
  state,
  opened = [],
  waiting,
}: {
  state: ChatState;
  opened?: string[];
  /** The service's answer to what the chat waits on, where a test's rows read from it. */
  waiting?: Waiting;
}) {
  const [open, setOpen] = useState(false);
  const pauses = { pauses: [], usage: [] };
  const doing = activity(toThread(state, [], [], new Map()).items);
  return (
    <header style={{ display: 'flex', justifyContent: 'space-between', position: 'relative' }}>
      <button type="button">Made-up neighbour</button>
      <Summary
        state={state}
        team={glances(state, doing, waiting)}
        open={open}
        onOpen={setOpen}
        pauses={pauses}
        onPause={() => {}}
        onOpenAgent={(agent) => opened.push(agent)}
      />
    </header>
  );
}

const stack = () =>
  [...document.querySelectorAll('.summary-button .initial')].map(
    (i) => `${i.textContent}${i.hasAttribute('data-busy') ? ' ring' : ''}`,
  );
const dots = () => document.querySelectorAll('.summary-button [data-needs]').length;

test('the button names who needs the person and who works, and stacks their initials: workers ringed, the dot once', async () => {
  const idle: ChatState = { ...empty, members: trio };
  const screen = await render(<Shown state={idle} />);
  // Nobody at work: the plain icon, so the button is never empty.
  await expect.element(screen.getByRole('button', { name: 'Summary', exact: true })).toBeVisible();
  expect(stack()).toEqual([]);
  expect(document.querySelector('.summary-button svg')).not.toBeNull();

  const working = { ...idle, turns: turns(turn('t1', 'b')) };
  await screen.rerender(<Shown state={working} />);
  await expect
    .element(screen.getByRole('button', { name: 'Summary: Made-up Member working', exact: true }))
    .toBeVisible();
  expect([stack(), dots()]).toEqual([['MM ring'], 0]);

  // An approval is asked inside a running turn: that agent needs the person, and has no ring.
  const needs = { ...idle, turns: turns(turn('t3', 'c')), approvals: [approval('t3')] };
  await screen.rerender(<Shown state={needs} />);
  await expect
    .element(screen.getByRole('button', { name: 'Summary: Made-up Third needs you', exact: true }))
    .toBeVisible();
  expect([stack(), dots()]).toEqual([['MT'], 1]);

  // Both: whoever needs the person sits last, and a question to the person counts as an approval does.
  const both: ChatState = {
    ...idle,
    turns: turns(turn('t1', 'a'), turn('t2', 'b'), turn('t3', 'c', { status: 'done' })),
    tasks: [task(9, null, { kind: 'person', giver: 'c', given_in: 't3', state: 'waiting' })],
  };
  await screen.rerender(<Shown state={both} />);
  const name = 'Summary: Made-up Third needs you, Made-up Lead and Made-up Member working';
  await expect.element(screen.getByRole('button', { name, exact: true })).toBeVisible();
  expect([stack(), dots()]).toEqual([['ML ring', 'MM ring', 'MT'], 1]);
});

test('five agents at work still fit the button as one compact stack', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: turns(...members.map((m) => turn(`t-${m.agent}`, m.agent))),
  };
  const screen = await render(<Shown state={state} />);
  const name = 'Summary: Made-up Lead, Made-up Member, Made-up Third, Kit and Lane working';
  const button = screen.getByRole('button', { name, exact: true });
  await expect.element(button).toBeVisible();
  expect(stack()).toEqual(['ML ring', 'MM ring', 'MT ring', 'K ring', 'L ring']);
  // Five 18px initials overlapped by 4px, and the button's 8px each side.
  expect(button.element().getBoundingClientRect().width).toBe(18 + 4 * 14 + 16);
});

test('the plan lists the current round: each step, who has it, who waits on the person, and how many are done', async () => {
  const state: ChatState = {
    ...empty,
    members: trio,
    turns: turns(turn('lead', 'a', { status: 'done', position: 0 }), turn('t3', 'c', { task: 2 })),
    approvals: [approval('t3')],
    tasks: [
      task(1, 'b', { state: 'done' }),
      task(2, 'c'),
      task(3, 'b'),
      task(4, 'b', { state: 'failed' }),
      task(5, 'c', { state: 'ready' }),
      task(6, 'z', { state: 'ready' }),
    ],
  };
  const screen = await render(<Shown state={state} />);
  await screen.getByRole('button', { name: /^Summary/ }).click();
  const plan = page.getByRole('region', { name: 'Plan' });
  await expect.element(plan.getByRole('heading')).toHaveTextContent('Plan1 of 6 done');
  await expect
    .element(plan.getByRole('progressbar', { name: 'Plan progress' }))
    .toHaveAttribute('aria-valuenow', '1');
  expect(
    plan
      .getByRole('button')
      .elements()
      .map((b) => b.textContent),
  ).toEqual([
    'Made-up task 1, done, Made-up Member',
    'Made-up task 2, in progress, Made-up Third · needs you',
    'Made-up task 3, in progress, Made-up Member',
    'Made-up task 4, failed, Made-up Member · failed',
    // Not started: the person isn't holding it up, though its owner waits on them.
    'Made-up task 5, not started, Made-up Third',
  ]);
  // A step of an agent that left the chat is listed, with no panel to open.
  const gone = document.querySelectorAll('.plan-step')[5];
  expect([gone.tagName, gone.textContent]).toEqual(['DIV', 'Made-up task 6, not started, ']);

  // No hand-off, no Plan section.
  await screen.rerender(<Shown state={{ ...state, tasks: [] }} />);
  await expect.element(plan).not.toBeInTheDocument();
});

test('the team is one line each: what it is doing and its tasks done, in order of who needs the person', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: turns(
      turn('t2', 'b'),
      turn('t3', 'c'),
      turn('t4', 'd', { status: 'done' }),
      turn('t5', 'e', { status: 'done' }),
    ),
    messages: [tool('m', 't2', 'Bash')],
    approvals: [approval('t3')],
    waits: [wait('a', 'b')],
    tasks: [
      task(1, 'b', { state: 'done', given_in: null }),
      task(2, 'b', { given_in: null }),
      task(3, 'b', { state: 'cancelled', given_in: null }),
      task(4, 'e', { state: 'done', given_in: null }),
    ],
  };
  // What the lead waits for is the service's answer; the two with nothing to do are free.
  const stands = (agent: string, state: 'waiting' | 'free', words: string) => ({
    agent,
    state,
    words,
    on: [],
    until_us: null,
  });
  const waiting: Waiting = {
    lines: [],
    agents: [
      stands('a', 'waiting', 'Waiting for Made-up Member'),
      stands('d', 'free', 'Ready'),
      stands('e', 'free', 'Ready'),
    ],
  };
  const screen = await render(<Shown state={state} waiting={waiting} />);
  await screen.getByRole('button', { name: /^Summary/ }).click();
  const summary = page.getByRole('dialog', { name: 'Summary' });
  await expect.element(summary.getByRole('heading')).toHaveTextContent('Team1 working · 1 needs you');
  const rows = () =>
    [...document.querySelectorAll('.agent-row')].map((row) =>
      [...row.children].map((part) => part.textContent).filter(Boolean),
    );
  // The running turn's activity is what its row in the timeline says; start times are in the panel.
  expect(rows()).toEqual([
    ['MT', 'Made-up Third', 'Needs your approval'],
    ['MM', 'Made-up Member', 'Running a command', '1/21 of 2 tasks done'],
    ['ML', 'Made-up Lead', 'Waiting for Made-up Member'],
    ['K', 'Kit', 'Ready'],
    ['L', 'Lane', 'Ready', '1/11 of 1 task done'],
  ]);
  // "1/2" is for the eye; a screen reader is told "1 of 2 tasks done", not a half.
  const count = document.querySelectorAll('.agent-count')[1];
  expect(count.querySelector('[aria-hidden="true"]')?.textContent).toBe('1/2');
  await expect
    .element(summary.getByRole('button', { name: 'Made-up Member Running a command 1 of 2 tasks done' }))
    .toBeVisible();
  expect(document.querySelectorAll('.summary .initial[data-busy]')).toHaveLength(1);
});

test('the button opens and closes it; Escape closes it and gives focus back; so does a press outside', async () => {
  const screen = await render(<Shown state={{ ...empty, members: trio }} />);
  const button = screen.getByRole('button', { name: 'Summary', exact: true });
  const summary = page.getByRole('dialog', { name: 'Summary' });
  await button.click();
  await expect.element(summary).toBeVisible();
  await button.click();
  await expect.element(summary).not.toBeInTheDocument();

  await button.click();
  await expect.element(summary).toBeVisible();
  await userEvent.keyboard('{Escape}');
  await expect.element(summary).not.toBeInTheDocument();
  await expect.element(button).toHaveFocus();

  await button.click();
  await expect.element(summary).toBeVisible();
  await screen.getByRole('button', { name: 'Made-up neighbour' }).click();
  await expect.element(summary).not.toBeInTheDocument();
});

test('a step or an agent opens that agent, and Summary closes', async () => {
  const opened: string[] = [];
  const state: ChatState = {
    ...empty,
    members: trio,
    turns: turns(turn('lead', 'a', { status: 'done', position: 0 })),
    tasks: [task(1, 'c')],
  };
  const screen = await render(<Shown state={state} opened={opened} />);
  const button = screen.getByRole('button', { name: /^Summary/ });
  const summary = page.getByRole('dialog', { name: 'Summary' });
  await button.click();
  await summary.getByRole('button', { name: /^Made-up task 1/ }).click();
  await expect.element(summary).not.toBeInTheDocument();
  await button.click();
  await summary.getByRole('button', { name: /Made-up Member/ }).click();
  await expect.element(summary).not.toBeInTheDocument();
  expect(opened).toEqual(['c', 'b']);
});
