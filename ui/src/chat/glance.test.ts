import { expect, test } from 'vitest';
import type { Member, Standing, Task, Turn, Waiting } from '../api';
import type { Mark } from '../components/marks';
import { glances, markOf } from './glance';
import { type ChatState, empty } from './state';
import { clock } from './team';

const members: Member[] = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
  { agent: 'b', name: 'Made-up Member', role: 'member' },
  { agent: 'c', name: 'Made-up Third', role: 'member' },
];
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
const stands = (agent: string, more: Partial<Standing> = {}): Standing => ({
  agent,
  name: 'Made-up',
  state: 'waiting',
  words: 'Waiting',
  what: null,
  task: null,
  since_us: null,
  on: [],
  until_us: null,
  failing: false,
  ...more,
});
const said = (...agents: Standing[]): Waiting => ({ lines: [], agents, gathr: [] });
const doing = (state: ChatState, waiting?: Waiting) =>
  Object.fromEntries(glances(state, new Map(), waiting).map((g) => [g.member.agent, g.doing]));

test('a face shows the most pressing of what the chat as it streams says and what the service answered', () => {
  // [what the chat says, the service's state, the mark shown]
  const table: [Parameters<typeof markOf>[0], Mark | undefined, Mark][] = [
    [{}, undefined, 'free'],
    [{ owed: true }, undefined, 'waiting'],
    [{ owed: true }, 'free', 'waiting'],
    [{}, 'working', 'free'],
    [{}, 'needs', 'needs'],
    [{}, 'cant', 'cant'],
    [{}, 'stopped', 'stopped'],
    [{ owed: true }, 'blocked', 'blocked'],
    [{ owed: true }, 'waiting', 'waiting'],
    [{ working: true }, 'stopped', 'working'],
    [{ working: true }, 'cant', 'working'],
    [{ working: true }, 'needs', 'needs'],
    [{ needs: true }, 'cant', 'needs'],
    [{ needs: true, working: true }, 'free', 'needs'],
  ];
  const shown = table.map(([local, service]) => markOf(local, service));
  expect(shown).toEqual(table.map(([, , mark]) => mark));
});

test('an agent that is not working says what the service says it waits for: held, out of usage, or for whom', () => {
  // The lead has a queued turn, the member an open task, the third nothing.
  const state: ChatState = {
    ...empty,
    members,
    turns: { t: turn('t', 'a', 'queued') },
    tasks: [task(1, 'b')],
  };
  // Until the answer comes, only whether anything is its to do.
  expect(doing(state)).toEqual({ a: 'Waiting', b: 'Waiting', c: 'Free' });

  const until = Date.now() + 3_600_000;
  const held = said(
    stands('a', { words: 'Held' }),
    stands('b', { words: 'Usage limit', until_us: until * 1000 }),
    stands('c', { state: 'free', words: 'Free' }),
  );
  expect(doing(state, held)).toEqual({
    a: 'Held',
    b: `Usage limit until ${clock(new Date(until).toISOString())}`,
    c: 'Free',
  });

  // Whom and what it waits for is one phrase in Summary, and what it waits on, its card's and its panel's.
  const since = Date.now() * 1000;
  const on = [
    { what: 'CI on #12', since_us: since },
    { what: 'Made-up Third', since_us: since },
  ];
  const named = said(stands('b', { words: 'Waiting for CI on #12 and Made-up Third', on }));
  const member = glances(state, new Map(), named).find((g) => g.member.agent === 'b');
  expect(member?.doing).toBe('Waiting for CI on #12 and Made-up Third');
  expect(member?.about).toBe('PR #12 checks and Made-up Third');

  // Nor does it hide a turn just queued for an agent it still calls free.
  const justQueued = { ...state, turns: { t: turn('t', 'c', 'queued') } };
  expect(doing(justQueued, held).c).toBe('Waiting');
  // The chat as it streams says who works: an answer a moment old never calls a working agent free.
  const working = { ...state, turns: { t: turn('t', 'c', 'running') } };
  expect(doing(working, held).c).toBe('Working');
});

test('a lead whose question is being discussed reads as working, not as asking the person', () => {
  const question = task(12, 'person', { kind: 'person', giver: 'a', state: 'held' });
  const state: ChatState = { ...empty, members, turns: { t: turn('t', 'a', 'running') }, tasks: [question] };
  const lead = (s: ChatState) => glances(s, new Map()).find((g) => g.member.agent === 'a');
  expect([lead(state)?.standing, lead(state)?.doing]).toEqual(['working', 'Working']);
  const back = { ...state, tasks: [{ ...question, state: 'ready' as const }] };
  expect([lead(back)?.standing, lead(back)?.doing]).toEqual(['needs', 'Asked you a question']);
});

test('an agent whose glance reads as before keeps its object, so its panel is not drawn again', () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: { t: turn('t', 'b', 'running') },
    tasks: [task(1, 'c', { state: 'done' })],
  };
  const first = glances(state, new Map([['b', { label: 'Reading a file' }]]));
  // The member moves on to a command; nothing about the others changed, though the state is a new object.
  const next = glances(
    { ...state, partial: { t: 'made-up' } },
    new Map([['b', { label: 'Running a command' }]]),
    undefined,
    first,
  );
  const of = (all: typeof first, agent: string) => all.find((g) => g.member.agent === agent);
  expect(of(next, 'a')).toBe(of(first, 'a'));
  expect(of(next, 'c')).toBe(of(first, 'c'));
  expect(of(next, 'b')).not.toBe(of(first, 'b'));
  expect(of(next, 'b')?.doing).toBe('Running a command');
});
