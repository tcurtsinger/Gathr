import { expect, test } from 'vitest';
import type { Message, Task, Turn } from '../api';
import { currentRound } from './plan';
import { type ChatState, empty } from './state';

const members = ['Mira', 'Rowan', 'Kit', 'Lane', 'Sol'].map((name, i) => ({
  agent: name.toLowerCase(),
  name,
  role: i === 0 ? ('supervisor' as const) : ('member' as const),
}));
const turn = (id: string, agent: string, position: number, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  position,
  ...more,
});
const task = (number: number, owner: string, given_in: string, more: Partial<Task> = {}): Task => ({
  number,
  chat: 'c',
  giver: 'mira',
  owner,
  given_in,
  kind: 'work',
  what: `Task ${number}`,
  state: 'ready',
  reason: null,
  after: [],
  created_us: 0,
  ...more,
});
const chat = (turns: Turn[], tasks: Task[], messages: Message[] = []): ChatState => ({
  ...empty,
  members,
  turns: Object.fromEntries(turns.map((t) => [t.id, t])),
  tasks,
  messages,
});

test("the round is the Supervisor's newest hand-off, with tasks given inside it, by task number", () => {
  const state = chat(
    [
      turn('first', 'mira', 0),
      turn('direct', 'lane', 1),
      turn('second', 'mira', 2),
      turn('rowan', 'rowan', 3, { task: 3 }),
    ],
    [
      task(1, 'rowan', 'first', { state: 'done' }),
      // A member's own hand-off in a direct exchange is not the Supervisor's plan.
      task(2, 'kit', 'direct', { giver: 'lane' }),
      task(3, 'rowan', 'second', { state: 'working' }),
      task(4, 'kit', 'second', { state: 'cancelled' }),
      // Rowan asks Sol to review from his own turn: still the second round.
      task(6, 'sol', 'rowan', { giver: 'rowan' }),
      task(5, 'lane', 'second'),
      // A question for the person is no step.
      task(7, 'mira', 'second', { kind: 'person_request', giver: 'lane' }),
      { ...task(8, 'mira', 'second', { kind: 'person' }), owner: null },
    ],
  );
  const round = currentRound(state);
  expect(round?.root).toBe('second');
  expect(round?.steps.map((s) => [s.task, s.owner, s.state])).toEqual([
    [3, 'Rowan', 'active'],
    [5, 'Lane', 'pending'],
    [6, 'Sol', 'pending'],
  ]);
  expect(round?.done).toBe(0);
});

test('a held step is at work; blocked, failed and stopped steps keep their own state, and none counts done', () => {
  const state = chat(
    [turn('ask', 'mira', 0)],
    [
      task(1, 'kit', 'ask', { state: 'working' }),
      task(2, 'rowan', 'ask', { state: 'held', reason: 'done, waiting on #12' }),
      task(3, 'sol', 'ask', { state: 'blocked', reason: 'Waiting for the replay fixture' }),
      task(4, 'rowan', 'ask', { state: 'failed', reason: 'Replay command failed (ref r-1)' }),
      task(5, 'sol', 'ask', { state: 'stopped' }),
      task(6, 'lane', 'ask', { state: 'done' }),
    ],
  );
  const round = currentRound(state);
  expect(round?.steps.map((s) => [s.agent, s.state])).toEqual([
    ['kit', 'active'],
    ['rowan', 'active'],
    ['sol', 'blocked'],
    ['rowan', 'failed'],
    ['sol', 'stopped'],
    ['lane', 'done'],
  ]);
  expect(round?.done).toBe(1);
});

test('a finished round stays until the person’s next message, then it is no longer the plan (ux-issues 19)', () => {
  const said = (id: string, turnId: string, role: Message['role']): Message => ({
    id,
    turn: turnId,
    time: '2026-11-10T09:00:00Z',
    role,
    kind: 'text',
    body: 'Made-up words',
  });
  const turns = [turn('ask', 'mira', 0), turn('rowan', 'rowan', 1, { task: 1 })];
  const tasks = [task(1, 'rowan', 'ask', { state: 'done' }), task(2, 'kit', 'ask', { state: 'failed' })];
  const asked = [said('m1', 'ask', 'user'), said('m2', 'ask', 'assistant')];
  const heard = [...turns, turn('hear', 'mira', 2, { answers: 'ask' })];
  const wrapUp = [...asked, said('m3', 'hear', 'gathr'), said('m4', 'hear', 'assistant')];
  const next = [...wrapUp, said('m5', 'later', 'user')];

  // Settled and heard: still shown, with its failure, until the person writes again.
  expect(currentRound(chat(heard, tasks, wrapUp))?.done).toBe(1);
  expect(currentRound(chat([...heard, turn('later', 'mira', 3)], tasks, next))).toBeUndefined();

  // Not finished, the person's messages don't hide it: the hear-back hasn't run, is still running, or a
  // task was restarted after it.
  expect(currentRound(chat([...turns, turn('later', 'mira', 3)], tasks, next))?.root).toBe('ask');
  const hearing = [...turns, turn('hear', 'mira', 2, { answers: 'ask', status: 'running' })];
  expect(currentRound(chat([...hearing, turn('later', 'mira', 3)], tasks, next))?.root).toBe('ask');
  const restarted = [tasks[0] as Task, task(2, 'kit', 'ask', { state: 'ready' })];
  expect(currentRound(chat([...heard, turn('later', 'mira', 3)], restarted, next))?.root).toBe('ask');
});

test('no round without a hand-off of the Supervisor', () => {
  expect(currentRound(chat([turn('ask', 'mira', 0)], []))).toBeUndefined();
});

test("G158: what the Supervisor hands on after the person's answer is a new round, though it runs on a member's task", () => {
  // Rowan, on task 1, gives Mira task 7; Mira, on 7, asks the person 8; the answer wakes Mira on 7 again,
  // and there she gives Kit task 9.
  const state = chat(
    [
      turn('ask', 'mira', 0),
      turn('rowan', 'rowan', 1, { task: 1, answers: 'ask' }),
      turn('mira7', 'mira', 2, { task: 7, answers: 'rowan' }),
      turn('woken', 'mira', 3, { task: 7, answers: 'mira7' }),
    ],
    [
      task(1, 'rowan', 'ask', { state: 'working' }),
      task(7, 'mira', 'rowan', { giver: 'rowan', state: 'working' }),
      { ...task(8, 'mira', 'mira7', { kind: 'person', state: 'done', heard_in: 'woken' }), owner: null },
      task(9, 'kit', 'woken'),
    ],
  );
  const round = currentRound(state);
  expect(round?.root).toBe('woken');
  expect(round?.steps.map((s) => s.task)).toEqual([9]);

  // The same turn not woken by an answer stays in the round its task belongs to.
  const unheard = { ...state, tasks: state.tasks.map((t) => ({ ...t, heard_in: null })) };
  expect(currentRound(unheard)?.root).toBe('ask');
  expect(currentRound(unheard)?.steps.map((s) => s.task)).toEqual([1, 7, 9]);
});

test("a task given in a turn another agent's message woke joins that agent's round; the Supervisor's own next turn starts one", () => {
  // Kit, on no task, answers Mira's turn (her message woke it) and gives Lane task 5.
  const woken = chat(
    [turn('ask', 'mira', 0), turn('told', 'kit', 1, { answers: 'ask' })],
    [task(1, 'rowan', 'ask'), task(5, 'lane', 'told', { giver: 'kit' })],
  );
  expect(currentRound(woken)?.root).toBe('ask');
  expect(currentRound(woken)?.steps.map((s) => s.task)).toEqual([1, 5]);

  // Mira hearing back on her own turn and handing on from there: her word, a round of its own.
  const next = chat(
    [turn('ask', 'mira', 0), turn('back', 'mira', 1, { answers: 'ask' })],
    [task(1, 'rowan', 'ask', { state: 'done' }), task(6, 'kit', 'back')],
  );
  expect(currentRound(next)?.root).toBe('back');
  expect(currentRound(next)?.steps.map((s) => s.task)).toEqual([6]);
});
