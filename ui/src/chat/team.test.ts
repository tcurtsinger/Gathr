import type { ThreadMessageLike } from '@assistant-ui/react';
import { expect, test } from 'vitest';
import type { Agent, Message, Task, Turn } from '../api';
import { type Custom, toThread } from './messages';
import { type ChatState, empty } from './state';
import { unfolded } from './TeamParts';

const people = ['Mira', 'Rowan', 'Kit', 'Lane', 'Sol'];
const agents: Agent[] = people.map((name) => ({
  id: name.toLowerCase(),
  name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto',
}));
const members = people.map((name, i) => ({
  agent: name.toLowerCase(),
  name,
  role: i === 0 ? ('supervisor' as const) : i === 4 ? ('reviewer' as const) : ('member' as const),
}));

let clock = 0;
const at = () => new Date(Date.UTC(2026, 10, 8, 9, 0, clock++)).toISOString();
const turn = (id: string, agent: string, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  created: at(),
  ...more,
});
const said = (turnId: string, role: Message['role'], text: string): Message => ({
  id: `${turnId}-${role}-${clock}`,
  turn: turnId,
  time: at(),
  role,
  kind: 'text',
  body: text,
});
const task = (number: number, owner: string, given_in: string, more: Partial<Task> = {}): Task => ({
  number,
  chat: 'c',
  giver: 'mira',
  owner,
  given_in,
  kind: 'work',
  what: `Task ${number}`,
  state: 'done',
  reason: null,
  after: [],
  ...more,
});

/** A round: the person asks Mira, Mira hands Rowan and Kit a task each, the person asks Mira something
 * meanwhile, and @Lane directly; the members' turns start after that. */
function round(): ChatState {
  clock = 0;
  const turns = [
    turn('ask', 'mira'),
    turn('mid', 'mira'),
    turn('direct', 'lane'),
    turn('rowan', 'rowan', { task: 1 }),
    turn('kit', 'kit', { task: 2 }),
  ];
  const messages = [
    said('ask', 'user', 'Fix the reconnect'),
    said('ask', 'assistant', 'Rowan and Kit, over to you'),
    said('mid', 'user', 'Keep the partial answer'),
    said('mid', 'assistant', 'Yes'),
    said('direct', 'user', '@Lane does Retry keep my draft?'),
    said('direct', 'assistant', 'It does'),
    said('rowan', 'gathr', 'Task #1 from Mira: Task 1'),
    said('rowan', 'assistant', 'Cursor fixed'),
    said('kit', 'gathr', 'Task #2 from Mira: Task 2'),
    said('kit', 'assistant', 'Checked'),
  ];
  return {
    ...empty,
    members,
    messages,
    turns: Object.fromEntries(turns.map((t) => [t.id, t])),
    tasks: [task(1, 'rowan', 'ask'), task(2, 'kit', 'ask', { state: 'working' })],
  };
}

const view = (state: ChatState) => toThread(state, agents, [], new Map());
const custom = (m: ThreadMessageLike) => (m.metadata?.custom ?? {}) as Custom;
const ids = (all: ThreadMessageLike[]) => all.map((m) => m.id ?? '');

test("members' work sits under the message that gave it, not where it fell in time", () => {
  const all = view(round());
  const order = ids(all).map((id) => id.replace(/-gathr-\d+$/, '-gathr'));
  expect(order).toEqual([
    expect.stringMatching(/^ask-user/),
    'a-ask',
    't-ask',
    'rowan-gathr',
    'a-rowan',
    'kit-gathr',
    'a-kit',
    expect.stringMatching(/^mid-user/),
    'a-mid',
    expect.stringMatching(/^direct-user/),
    'a-direct',
  ]);
  const byId = (id: string) => custom(all.find((m) => m.id === id) as ThreadMessageLike);
  expect(byId('t-ask').head).toMatchObject({ root: 'ask', text: 'Kit working · Rowan done', done: false });
  expect(byId('a-rowan')).toMatchObject({ thread: 'ask', agent: 'Rowan', role: 'member' });
  expect(byId('a-direct').thread).toBeUndefined();
  // Gathr's message that started a task is its assignment line, never an agent's reply.
  const assignment = all.find((m) => m.id?.startsWith('rowan-gathr')) as ThreadMessageLike;
  expect(assignment.role).toBe('system');
  expect(custom(assignment)).toMatchObject({
    thread: 'ask',
    first: true,
    assignment: { from: 'Mira', to: 'Rowan', what: 'Task 1' },
  });
});

test('a task a member gave sits in the same thread, and a turn nobody gave a task to has none', () => {
  const state = round();
  state.turns.sub = turn('sub', 'lane', { task: 3 });
  state.tasks.push(task(3, 'lane', 'rowan', { giver: 'rowan' }));
  state.messages.push(said('sub', 'assistant', 'Sub-task done'));
  const all = view(state);
  expect(custom(all.find((m) => m.id === 'a-sub') as ThreadMessageLike).thread).toBe('ask');
  expect(all.filter((m) => custom(m).head)).toHaveLength(1);
});

test("the lead's last word links back, folds the thread, and shows who agreed; a late reply says so in place", () => {
  const state = round();
  state.tasks = [task(1, 'rowan', 'ask'), task(2, 'kit', 'ask'), task(3, 'sol', 'ask', { kind: 'review' })];
  state.turns.sol = turn('sol', 'sol', { task: 3 });
  state.messages.push(said('sol', 'assistant', 'No findings'));
  state.turns.back = turn('back', 'mira', { answers: 'ask' });
  state.messages.push(said('back', 'gathr', 'The 3 tasks you gave together are settled'));
  state.messages.push(said('back', 'assistant', 'All done'));
  let all = view(state);
  expect(all.some((m) => m.id?.startsWith('back-gathr'))).toBe(false);
  expect(custom(all.at(-1) as ThreadMessageLike)).toMatchObject({ back: 'ask', agreed: ['Sol'] });
  const head = custom(all.find((m) => m.id === 't-ask') as ThreadMessageLike).head;
  expect(head).toMatchObject({ done: true, text: 'Rowan, Kit and Sol done' });
  expect(ids(unfolded(all, new Map()))).not.toContain('a-rowan');
  expect(ids(unfolded(all, new Map([['ask', true]])))).toContain('a-rowan');

  state.turns.late = turn('late', 'lane', { task: 4 });
  state.tasks.push(task(4, 'lane', 'ask'));
  state.messages.push(said('late', 'assistant', 'Retry keeps it'));
  all = view(state);
  expect(custom(all.find((m) => m.id === 't-ask') as ThreadMessageLike).head?.text).toMatch(
    /^Lane replied at \d\d:\d\d · Rowan, Kit and Sol done$/,
  );
  expect(all.at(-1)?.id).toBe('a-back');
});

test("Gathr's wake-up reads as Gathr's, not as the agent's reply", () => {
  const state = round();
  state.turns.woke = turn('woke', 'mira');
  state.messages.push(said('woke', 'gathr', 'Your wait on CI on #12 ended'));
  state.messages.push(said('woke', 'assistant', 'CI passed'));
  const all = view(state);
  const wake = all.find((m) => m.id?.startsWith('woke-gathr')) as ThreadMessageLike;
  expect(wake.role).toBe('system');
  expect(custom(wake).gathr).toBe('Your wait on CI on #12 ended');
  expect(all.find((m) => m.id === 'a-woke')?.content).toEqual([{ type: 'text', text: 'CI passed' }]);
});

test("the lead's word mid-round links back but doesn't fold the thread while work is open", () => {
  const state = round();
  state.turns.woke = turn('woke', 'mira', { answers: 'ask' });
  state.messages.push(said('woke', 'gathr', 'Your wait on CI on #12 ended'));
  state.messages.push(said('woke', 'assistant', 'CI passed; Kit is still on it'));
  const all = view(state);
  expect(custom(all.find((m) => m.id === 'a-woke') as ThreadMessageLike).back).toBe('ask');
  expect(custom(all.find((m) => m.id === 't-ask') as ThreadMessageLike).head?.done).toBe(false);
});
