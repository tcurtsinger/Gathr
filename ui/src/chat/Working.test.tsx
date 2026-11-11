import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Task, Turn } from '../api';
import { type ChatState, empty } from './state';
import { leadWait, TeamWait, working } from './Working';

const members = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' as const },
  { agent: 'kit', name: 'Kit', role: 'member' as const },
  { agent: 'lane', name: 'Lane Ortiz', role: 'member' as const },
];
const turn = (id: string, agent: string, status: Turn['status'], task?: number): Turn => ({
  id,
  agent,
  status,
  why: null,
  ref: id,
  task,
});
const task = (number: number, owner: string, created_us: number, state: Task['state'] = 'working'): Task => ({
  number,
  chat: 'c',
  giver: 'mira',
  owner,
  given_in: 'ask',
  kind: 'work',
  what: `Task ${number}`,
  state,
  reason: null,
  after: [],
  created_us,
});
const nineFifteen = Date.UTC(2026, 10, 10, 9, 15) * 1000;

/** Mira handed Kit and Lane a task each; Kit's turn runs, Lane's waits in the queue. */
function round(): ChatState {
  return {
    ...empty,
    members,
    turns: {
      ask: turn('ask', 'mira', 'done'),
      kit: turn('kit', 'kit', 'running', 1),
      lane: turn('lane', 'lane', 'queued', 2),
    },
    tasks: [task(1, 'kit', nineFifteen), task(2, 'lane', nineFifteen + 60_000_000, 'ready')],
  };
}

test("only members whose turn on a hand-off runs are working; the Supervisor waits on its round's open tasks", async () => {
  const state = round();
  expect(working(state).map((m) => m.name)).toEqual(['Kit']);
  expect(leadWait(state)).toMatchObject({ names: ['Kit', 'Lane Ortiz'], since: nineFifteen });

  const screen = await render(<TeamWait state={state} />);
  await expect.element(screen.getByRole('status')).toBeVisible();
  expect(screen.getByRole('status').element().textContent).toMatch(/^Waiting on teamK· \d/);

  // The Supervisor at work again isn't waiting; nor is it once its tasks are settled.
  expect(
    leadWait({ ...state, turns: { ...state.turns, back: turn('back', 'mira', 'running') } }),
  ).toBeUndefined();
  const settled = state.tasks.map((t) => ({ ...t, state: 'done' as const }));
  expect(leadWait({ ...state, tasks: settled })).toBeUndefined();
});
