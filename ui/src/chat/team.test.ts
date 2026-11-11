import type { ThreadMessageLike } from '@assistant-ui/react';
import { expect, test } from 'vitest';
import type { Agent, Message, Task, Turn } from '../api';
import { type Custom, toThread } from './messages';
import { apply, type ChatState, empty } from './state';

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
  created_us: 0,
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
    turns: Object.fromEntries(turns.map((t, position) => [t.id, { ...t, position }])),
    tasks: [task(1, 'rowan', 'ask'), task(2, 'kit', 'ask', { state: 'working' })],
  };
}

const view = (state: ChatState) => toThread(state, agents, [], new Map());
const custom = (m: ThreadMessageLike) => (m.metadata?.custom ?? {}) as Custom;
const ids = (all: ThreadMessageLike[]) => all.map((m) => (m.id ?? '').replace(/-(user|gathr)-\d+$/, '-$1'));
const byId = (all: ThreadMessageLike[], id: string) =>
  custom(all.find((m) => m.id === id) as ThreadMessageLike);
const text = (m: ThreadMessageLike | undefined) =>
  ((m?.content ?? []) as readonly { text?: string; toolName?: string }[])
    .map((p) => p.text ?? p.toolName)
    .join(' | ');

test("members' work on a hand-off is in the team column; the person's and the lead's words stay in the main one", () => {
  const { main, team } = view(round());
  expect(ids(main)).toEqual(['ask-user', 'a-ask', 'mid-user', 'a-mid', 'direct-user', 'a-direct']);
  expect(ids(team)).toEqual(['rowan-gathr', 'a-rowan', 'kit-gathr', 'a-kit']);
  expect(byId(team, 'a-rowan')).toMatchObject({ agent: 'Rowan', role: 'member', time: expect.any(String) });
  // Gathr's message that started a task is its assignment line, never an agent's reply.
  const assignment = team[0];
  expect(assignment.role).toBe('system');
  expect(custom(assignment)).toMatchObject({ assignment: { from: 'Mira', to: 'Rowan', what: 'Task 1' } });
});

test("replies that finish out of order keep their turns' order in the team column, as a reload shows them", () => {
  const state = round();
  // Rowan's turn started first; Kit's reply was saved first.
  const rowan = state.messages.filter((m) => m.turn === 'rowan');
  state.messages = [...state.messages.filter((m) => m.turn !== 'rowan'), ...rowan];
  expect(ids(view(state).team)).toEqual(['rowan-gathr', 'a-rowan', 'kit-gathr', 'a-kit']);
});

test("two turns started live keep the service's order and show when they were made, whatever order their updates arrive in", () => {
  const state = round();
  const live = (turn: string, agent: string, task: number, position: number, created: string) =>
    ({ type: 'turn', chat: 'c', turn, status: 'running', agent, task, position, created }) as const;
  // Lane's task turn started before Sol's, but Sol's update arrived first; neither has saved a word.
  const started = [
    live('sol', 'sol', 4, 7, '2026-11-08T09:05:01Z'),
    live('lane', 'lane', 3, 6, '2026-11-08T09:05:00Z'),
  ];
  let next = started.reduce(apply, state);
  next.tasks.push(task(3, 'lane', 'ask', { state: 'working' }), task(4, 'sol', 'ask', { state: 'working' }));
  next = { ...next, partial: { sol: 'Sol starts', lane: 'Lane starts' } };
  const { team } = view(next);
  expect(ids(team).slice(-2)).toEqual(['a-lane', 'a-sol']);
  expect(byId(team, 'a-sol').time).toBe('2026-11-08T09:05:01Z');
});

test("two turns' messages saved interleaved are one reply each, in each column", () => {
  const state = round();
  // Rowan and Kit work at once in the team column; Mira, woken by Gathr, and Lane at once in the main one.
  state.turns.woke = turn('woke', 'mira');
  const tool = (turnId: string, id: string): Message => ({
    id,
    turn: turnId,
    time: '2026-11-08T09:01:00Z',
    role: 'assistant',
    kind: 'tool',
    body: { tool: 'Bash', status: 'ok' },
  });
  state.messages.push(
    tool('rowan', 'r1'),
    tool('kit', 'k1'),
    said('rowan', 'assistant', 'Rowan again'),
    said('kit', 'assistant', 'Kit again'),
    said('woke', 'gathr', 'Your wait on CI on #12 ended'),
    said('woke', 'assistant', 'CI passed'),
    said('direct', 'assistant', 'Lane again'),
    said('woke', 'assistant', 'Mira again'),
  );
  const { main, team } = view(state);
  expect(ids(team)).toEqual(['rowan-gathr', 'a-rowan', 'kit-gathr', 'a-kit']);
  expect(team[1].content).toHaveLength(3);
  expect(text(team[3])).toBe('Checked | Bash | Kit again');
  expect(ids(main).slice(4)).toEqual(['direct-user', 'a-direct', 'woke-gathr', 'a-woke']);
  expect(text(main[5])).toBe('It does | Lane again');
  expect(text(main[7])).toBe('CI passed | Mira again');
});

test("a person's message sent into a running turn still splits that reply where it was sent", () => {
  const state = round();
  state.turns.mid.status = 'running';
  state.messages.push(said('ask', 'user', 'Also keep the draft'), said('mid', 'assistant', 'Will do'));
  state.partial = { mid: 'Streaming on' };
  const { main } = view(state);
  expect(ids(main).slice(2)).toEqual(['mid-user', 'a-mid', 'direct-user', 'a-direct', 'ask-user', 'a-mid-1']);
  expect(text(main.at(-1))).toBe('Will do | Streaming on');
  // A person's message in the main column never splits a member's reply in the team column.
  state.messages.push(said('kit', 'assistant', 'Kit carries on'));
  expect(ids(view(state).team)).toEqual(['rowan-gathr', 'a-rowan', 'kit-gathr', 'a-kit']);
});

test("the person's message to a member working on a task is in the main column; the member's reply stays whole in the team one", () => {
  const state = round();
  state.turns.kit.status = 'running';
  // The service saves a message sent into a running turn on that turn.
  state.messages.push(said('kit', 'user', '@Kit keep the shortcuts'), said('kit', 'assistant', 'Kept'));
  state.partial = { kit: 'Still checking' };
  const { main, team } = view(state);
  expect(ids(main).slice(-1)).toEqual(['kit-user']);
  expect(ids(team)).toEqual(['rowan-gathr', 'a-rowan', 'kit-gathr', 'a-kit']);
  expect(text(team.at(-1))).toBe('Checked | Kept | Still checking');
});

test('a task a member gave is in the team column, and a turn nobody gave a task to stays in the main one', () => {
  const state = round();
  state.turns.sub = turn('sub', 'lane', { task: 3 });
  state.tasks.push(task(3, 'lane', 'rowan', { giver: 'rowan' }));
  state.messages.push(said('sub', 'assistant', 'Sub-task done'));
  const { main, team } = view(state);
  expect(ids(team)).toContain('a-sub');
  expect(ids(main)).toContain('a-direct');
});

test("what Gathr told the lead when a round settled isn't shown; the lead's word is, in the main column", () => {
  const state = round();
  state.turns.back = turn('back', 'mira', { answers: 'ask' });
  state.messages.push(said('back', 'gathr', 'The 2 tasks you gave are settled'));
  state.messages.push(said('back', 'assistant', 'All done'));
  const { main, team } = view(state);
  expect(ids(main).slice(-1)).toEqual(['a-back']);
  expect(ids([...main, ...team])).not.toContain('back-gathr');
});

test("the person's answers the lead heard show as one quiet line each where it heard them; its round report doesn't", () => {
  const state = round();
  // The turn that handed out tasks 1 and 2 also asked the person 5 and 6; the person answered 5 and cancelled 6.
  const asked = { owner: null, giver: 'mira', kind: 'person' as const };
  state.tasks.push(
    task(5, 'mira', 'ask', { ...asked, what: 'Which notice?', reason: 'B: two sections', heard_in: 'heard' }),
    task(6, 'mira', 'ask', { ...asked, state: 'cancelled', heard_in: 'heard' }),
  );
  state.turns.back = turn('back', 'mira', { answers: 'ask' });
  state.turns.heard = turn('heard', 'mira', { answers: 'ask' });
  state.messages.push(
    said('back', 'gathr', 'The 2 tasks you gave are settled'),
    said('heard', 'gathr', 'The person answered:\n- #5 Which notice?: B: two sections\nContinue.'),
  );
  const { main } = view(state);
  expect(ids(main)).not.toContain('back-gathr');
  const line = main.find((m) => m.id?.startsWith('heard-gathr')) as ThreadMessageLike;
  expect(line.role).toBe('system');
  expect(custom(line)).toEqual({
    answered: [{ line: 'You answered #5 · B: two sections', asked: 'Which notice?' }],
  });
});

test("Gathr's wake-up reads as Gathr's, not as the agent's reply", () => {
  const state = round();
  state.turns.woke = turn('woke', 'mira');
  state.messages.push(said('woke', 'gathr', 'Your wait on CI on #12 ended'));
  state.messages.push(said('woke', 'assistant', 'CI passed'));
  const { main } = view(state);
  const wake = main.find((m) => m.id?.startsWith('woke-gathr')) as ThreadMessageLike;
  expect(wake.role).toBe('system');
  expect(custom(wake).gathr).toBe('Your wait on CI on #12 ended');
  expect(main.find((m) => m.id === 'a-woke')?.content).toEqual([{ type: 'text', text: 'CI passed' }]);
});

test('a member hearing back on a task it gave stays in the team column', () => {
  const state = round();
  state.turns.sub = turn('sub', 'lane', { task: 3, answers: 'rowan' });
  state.tasks.push(task(3, 'lane', 'rowan', { giver: 'rowan' }));
  state.messages.push(said('sub', 'assistant', 'Sub-task done'));
  state.turns.heard = turn('heard', 'rowan', { answers: 'rowan' });
  state.messages.push(said('heard', 'gathr', 'The task you gave is settled'));
  state.messages.push(said('heard', 'assistant', 'Lane finished it; my part is done too'));
  const { team } = view(state);
  expect(ids(team).slice(-3)).toEqual(['a-sub', 'heard-gathr', 'a-heard']);
});
