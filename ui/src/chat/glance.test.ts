import { expect, test } from 'vitest';
import type { Member, Pauses, Task, Turn, Wait } from '../api';
import { glances } from './glance';
import type { Run } from './pauses';
import { type ChatState, empty } from './state';
import { clock } from './team';

const members: Member[] = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
  { agent: 'b', name: 'Made-up Member', role: 'member' },
  { agent: 'c', name: 'Made-up Third', role: 'member' },
];
const running: Run = { state: 'running', by: null, scope: null };
const none: Pauses = { pauses: [], usage: [] };
const turn = (id: string, agent: string, status: Turn['status']): Turn => ({
  id,
  agent,
  status,
  why: null,
  ref: id,
});
const task = (number: number, owner: string, more: Partial<Task> = {}): Task => ({
  number,
  chat: 'c',
  giver: null,
  owner,
  given_in: null,
  kind: 'work',
  what: `Made-up task ${number}`,
  state: 'ready',
  reason: null,
  after: [],
  created_us: 0,
  ...more,
});
const wait = (agent: string, kind: string, target: string): Wait => ({
  id: `w-${agent}-${kind}`,
  agent,
  task: null,
  kind,
  target,
  state: 'open',
  created_ts: 0,
});
const doing = (state: ChatState, pauses = none, run = running) =>
  Object.fromEntries(glances(state, pauses, run, new Map()).map((g) => [g.member.agent, g.doing]));

test('an agent that is not working says why: held or paused, stopped, out of usage, or what it waits for', () => {
  // The lead has a turn the pause holds, the member an open task, the third nothing.
  const state: ChatState = {
    ...empty,
    members,
    turns: { t: turn('t', 'a', 'queued') },
    tasks: [task(1, 'b')],
  };
  const chatPaused: Run = { state: 'paused', by: null, scope: { scope: 'chat', target: 'c' } };
  expect(doing(state, none, chatPaused)).toEqual({ a: 'Held', b: 'Held', c: 'Paused' });
  expect(doing(state, none, { ...chatPaused, state: 'stopped' })).toEqual({
    a: 'Stopped',
    b: 'Stopped',
    c: 'Stopped',
  });
  // One agent paused alone: the others carry on. Work with no named wait is just Waiting.
  const onePaused: Pauses = {
    pauses: [{ scope: 'agent', target: 'c', state: 'paused', by: null }],
    usage: [],
  };
  expect(doing(state, onePaused)).toEqual({ a: 'Waiting', b: 'Waiting', c: 'Paused' });

  const until = Date.now() + 3_600_000;
  const usage: Pauses = { pauses: [], usage: [{ account: 'x', until_us: until * 1000, agents: ['b'] }] };
  expect(doing(state, usage).b).toBe(`Usage limit until ${clock(new Date(until).toISOString())}`);

  // Waits on CI and on a PR are named as the service names them.
  const waiting = { ...state, waits: [wait('b', 'ci', '12'), wait('c', 'pr', '34')] };
  expect(doing(waiting)).toMatchObject({
    b: 'Waiting for CI on #12',
    c: 'Waiting for #34 to merge or close',
  });
});

test('an agent whose glance reads as before keeps its object, so its panel is not drawn again', () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: { t: turn('t', 'b', 'running') },
    tasks: [task(1, 'c', { state: 'done' })],
  };
  const first = glances(state, none, running, new Map([['b', 'Reading a file']]));
  // The member moves on to a command; nothing about the others changed, though the state is a new object.
  const next = glances(
    { ...state, partial: { t: 'made-up' } },
    none,
    running,
    new Map([['b', 'Running a command']]),
    first,
  );
  const of = (all: typeof first, agent: string) => all.find((g) => g.member.agent === agent);
  expect(of(next, 'a')).toBe(of(first, 'a'));
  expect(of(next, 'c')).toBe(of(first, 'c'));
  expect(of(next, 'b')).not.toBe(of(first, 'b'));
  expect(of(next, 'b')?.doing).toBe('Running a command');
});
