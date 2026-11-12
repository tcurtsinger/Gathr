// The chat as one timeline (0037): where each block sits, what a turn folds into its row, who talks to
// whom, and which of Gathr's notes become one-line events.
import type { ThreadMessageLike } from '@assistant-ui/react';
import { expect, test } from 'vitest';
import type { Agent, Message, Task, ToolBody, Turn } from '../api';
import { duration } from './fold';
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
const tool = (
  turnId: string,
  id: string,
  name: string,
  more: Partial<ToolBody> = {},
  time = at(),
): Message => ({
  id,
  turn: turnId,
  time,
  role: 'assistant',
  kind: 'tool',
  body: { tool: name, status: 'ok', ...more },
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
    // A turn on a task answers the turn that gave it, as the service links them.
    turn('rowan', 'rowan', { task: 1, answers: 'ask' }),
    turn('kit', 'kit', { task: 2, answers: 'ask' }),
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
const custom = (m: ThreadMessageLike | undefined) => (m?.metadata?.custom ?? {}) as Custom;
const ids = (all: ThreadMessageLike[]) => all.map((m) => (m.id ?? '').replace(/-(user|gathr)-\d+$/, '-$1'));
const byId = (all: ThreadMessageLike[], id: string) => all.find((m) => m.id === id);
const text = (m: ThreadMessageLike | undefined) =>
  ((m?.content ?? []) as readonly { text?: string }[]).map((p) => p.text).join(' | ');
const events = (all: ThreadMessageLike[]) => all.flatMap((m) => custom(m).events ?? []);

test('the chat is one list in order: the person, the lead and each member where its turn began', () => {
  const { items } = view(round());
  expect(ids(items)).toEqual([
    'ask-user',
    'a-ask',
    'mid-user',
    'a-mid',
    'direct-user',
    'a-direct',
    'a-rowan',
    'a-kit',
  ]);
  // A member's block is headed by the task it works on; Gathr's message that gave it isn't drawn.
  expect(custom(byId(items, 'a-rowan'))).toMatchObject({ agent: 'Rowan', role: 'member', on: 'Task 1' });
  expect(custom(byId(items, 'a-ask')).on).toBeUndefined();
});

test("replies that finish out of order keep their turns' order, as a reload shows them", () => {
  const state = round();
  // Rowan's turn started first; Kit's reply was saved first.
  const rowan = state.messages.filter((m) => m.turn === 'rowan');
  state.messages = [...state.messages.filter((m) => m.turn !== 'rowan'), ...rowan];
  expect(ids(view(state).items).slice(-2)).toEqual(['a-rowan', 'a-kit']);
});

test("two turns started live keep the service's order, whatever order their updates arrive in", () => {
  const state = round();
  const live = (id: string, agent: string, n: number, position: number, created: string) =>
    ({ type: 'turn', chat: 'c', turn: id, status: 'running', agent, task: n, position, created }) as const;
  // Lane's task turn started before Sol's, but Sol's update arrived first; neither has saved a word.
  const started = [
    live('sol', 'sol', 4, 7, '2026-11-08T09:05:01Z'),
    live('lane', 'lane', 3, 6, '2026-11-08T09:05:00Z'),
  ];
  let next = started.reduce(apply, state);
  next.tasks.push(task(3, 'lane', 'ask', { state: 'working' }), task(4, 'sol', 'ask', { state: 'working' }));
  next = { ...next, partial: { sol: 'Sol starts', lane: 'Lane starts' } };
  const { items } = view(next);
  expect(ids(items).slice(-2)).toEqual(['a-lane', 'a-sol']);
  expect(custom(byId(items, 'a-sol')).time).toBe('2026-11-08T09:05:01Z');
});

test('turns streaming at once never interleave, and a late reply lands in its own turn’s block', () => {
  const state = round();
  state.messages.push(
    tool('rowan', 'r1', 'Bash'),
    tool('kit', 'k1', 'Bash'),
    said('rowan', 'assistant', 'Rowan again'),
    said('kit', 'assistant', 'Kit again'),
    // The lead, whose turn began first, writes once more after its members.
    said('ask', 'assistant', 'Both are on it'),
  );
  const { items } = view(state);
  expect(ids(items)).toEqual([
    'ask-user',
    'a-ask',
    'mid-user',
    'a-mid',
    'direct-user',
    'a-direct',
    'a-rowan',
    'a-kit',
  ]);
  expect(text(byId(items, 'a-ask'))).toBe('Both are on it');
  expect(text(byId(items, 'a-rowan'))).toBe('Rowan again');
  expect(text(byId(items, 'a-kit'))).toBe('Kit again');
});

test('a turn waiting to start stays where its message was sent, and what comes after it keeps its place', () => {
  const state = round();
  // Held by a pause: no place among the turns yet.
  state.turns.held = turn('held', 'mira', { status: 'queued', held: 'pause' });
  state.turns.late = { ...turn('late', 'lane'), position: 5 };
  const note: Message = { ...said('kit', 'assistant', 'Kit to Rowan'), id: 'note-1', to: 'rowan' };
  state.messages.push(said('held', 'user', 'One more thing'), note, said('late', 'assistant', 'Lane, later'));
  expect(ids(view(state).items).slice(-4)).toEqual(['a-kit', 'held-user', 'note-1', 'a-late']);
});

test("a person's message sent into a running turn splits that turn's block where it was sent", () => {
  const state = round();
  state.turns.kit.status = 'running';
  state.messages.push(said('kit', 'user', '@Kit keep the shortcuts'), said('kit', 'assistant', 'Kept'));
  state.partial = { kit: 'Still checking' };
  const { items } = view(state);
  expect(ids(items).slice(-3)).toEqual(['a-kit', 'kit-user', 'a-kit-1']);
  expect(text(byId(items, 'a-kit'))).toBe('Checked');
  // What it said before the stream is mid-work now; the stream is its message.
  expect(custom(items.at(-1)).work?.steps).toMatchObject([{ say: 'Kept' }]);
  expect(text(items.at(-1))).toBe('Still checking');
});

test('a finished turn folds what it did into one row, and its last text is the message', () => {
  const state = round();
  const t0 = Date.UTC(2026, 10, 8, 10, 0, 0);
  const iso = (s: number) => new Date(t0 + s * 1000).toISOString();
  state.turns.lead = { ...turn('lead', 'mira', { created: iso(0) }), position: 9 };
  state.tasks.push(
    task(7, 'kit', 'lead', { what: 'Open the settings PR' }),
    task(8, 'rowan', 'lead', { what: 'Bring the fix up to date' }),
  );
  state.messages.push(
    tool('lead', 't1', 'Read', { duration_ms: 400 }, iso(1)),
    tool('lead', 't2', 'Bash', { status: 'error', duration_ms: 27_000 }, iso(2)),
    { ...said('lead', 'assistant', 'Two tasks are stale.'), time: iso(40) },
    tool('lead', 't3', 'mcp__gathr__list_tasks', {}, iso(41)),
    tool('lead', 't4', 'Glob', {}, iso(42)),
    tool('lead', 't5', 'mcp__gathr__give_task', {}, iso(50)),
    tool('lead', 't6', 'mcp__gathr__give_task', { duration_ms: 9_000 }, iso(100)),
    { ...said('lead', 'assistant', 'Two things are left.'), time: iso(109) },
  );
  const block = byId(view(state).items, 'a-lead');
  expect(text(block)).toBe('Two things are left.');
  const work = custom(block).work;
  expect(work).toMatchObject({
    running: false,
    label: 'Worked for 1m 49s',
    notes: ['6 steps', 'gave 2 tasks'],
  });
  expect(work?.steps).toEqual([
    { id: 't1', label: 'Read a file', state: 'ok', time: '' },
    { id: 't2', label: 'Ran a command', state: 'error', time: '27s' },
    { id: expect.any(String), say: 'Two tasks are stale.' },
    { id: 't3', label: 'Listed tasks', state: 'ok', time: '' },
    { id: 't4', label: 'Glob', state: 'ok', time: '' },
    { id: 't5', label: 'Gave Kit a task', detail: '#7 Open the settings PR', task: 7, state: 'ok', time: '' },
    {
      id: 't6',
      label: 'Gave Rowan a task',
      detail: '#8 Bring the fix up to date',
      task: 8,
      state: 'ok',
      time: '9s',
    },
  ]);
});

test('a running turn’s row says what the agent is doing now; text streaming last is its message', () => {
  const state = round();
  state.turns.kit.status = 'running';
  state.messages.push(tool('kit', 'k1', 'Edit'), tool('kit', 'k2', 'Shell', { status: 'running' }));
  const running = custom(byId(view(state).items, 'a-kit')).work;
  expect(running).toMatchObject({ running: true, label: 'Running a command', notes: ['2 steps'] });
  expect(running?.steps[0]).toEqual({ id: expect.any(String), say: 'Checked' });
  expect(text(byId(view(state).items, 'a-kit'))).toBe('');

  // The command ended and it writes: the row stays, the words show as its message.
  state.messages[state.messages.length - 1] = tool('kit', 'k2', 'Shell');
  state.partial = { kit: 'All 360 pass' };
  const block = byId(view(state).items, 'a-kit');
  expect(custom(block).work).toMatchObject({ running: true, label: 'Working', notes: ['2 steps'] });
  expect(text(block)).toBe('All 360 pass');
});

test('a turn that only wrote once has no row', () => {
  const { items } = view(round());
  expect(custom(byId(items, 'a-rowan')).work).toBeUndefined();
  expect(text(byId(items, 'a-rowan'))).toBe('Cursor fixed');
});

test('a row’s times read as a person says them, and nothing under a second shows', () => {
  expect([400, 9_000, 109_000, 365_000, 3_900_000].map(duration)).toEqual([
    '',
    '9s',
    '1m 49s',
    '6m 05s',
    '1h 5m',
  ]);
});

test('messages between agents are their own block, shared while they follow each other; Gathr’s copy stays hidden', () => {
  const state = round();
  // Kit asks Mira mid-turn, Mira answers from her own turn; later Mira writes to Rowan.
  const ask: Message = { ...said('kit', 'assistant', 'Is 30 seconds fine?'), id: 'ask-1', to: 'mira' };
  state.turns.told = { ...turn('told', 'mira', { answers: 'kit' }), position: 5 };
  const copy: Message = {
    ...said('told', 'gathr', 'Kit wrote to you: Is 30 seconds fine?'),
    copy_of: 'ask-1',
  };
  const answer: Message = { ...said('told', 'assistant', 'Yes, 30 seconds.'), id: 'answer-1', to: 'kit' };
  state.messages.push(
    ask,
    copy,
    tool('told', 's1', 'mcp__gathr__message'),
    answer,
    said('told', 'assistant', 'Kit has its answer'),
  );
  const later: Message = { ...said('told', 'assistant', 'Rowan, rebase first'), id: 'later-1', to: 'rowan' };
  state.messages.push(later);
  const { items, find } = view(state);

  // Mira's block follows the answer she sent before writing anything else, so the two read as one talk.
  expect(ids(items).slice(-4)).toEqual(['a-kit', 'ask-1', 'a-told', 'later-1']);
  const talk = byId(items, 'ask-1');
  expect(custom(talk).exchange).toMatchObject([
    { from: 'Kit', to: 'Mira' },
    { from: 'Mira', to: 'Kit' },
  ]);
  expect(text(talk)).toBe('Is 30 seconds fine? | Yes, 30 seconds.');
  expect(custom(byId(items, 'later-1')).exchange).toMatchObject([{ from: 'Mira', to: 'Rowan' }]);
  expect(text(byId(items, 'a-told'))).toBe('Kit has its answer');
  expect(custom(byId(items, 'a-told')).work?.steps).toMatchObject([{ label: 'Sent a message' }]);
  // A search hit on Gathr's copy opens at the message it carries.
  expect(find(copy.id)).toEqual({ id: 'ask-1', text: 'Is 30 seconds fine?', nth: 0 });
  expect(find('answer-1')).toEqual({ id: 'ask-1', text: 'Yes, 30 seconds.', nth: 0 });
});

test('Gathr’s notes are one-line events: a task heard back, the person’s answer, a question, a wake-up', () => {
  const state = round();
  const asked = { owner: null, giver: 'mira', kind: 'person' as const };
  state.tasks[0].reason = 'Approved at head 9a41c07';
  state.tasks.push(
    task(5, 'mira', 'ask', { ...asked, what: 'Which notice?', reason: 'B: two sections', heard_in: 'heard' }),
    task(6, 'mira', 'ask', { ...asked, state: 'cancelled', heard_in: 'heard' }),
    task(9, 'mira', 'heard', { ...asked, what: 'Ship it?', why: 'Two halves', state: 'ready' }),
  );
  state.turns.back = { ...turn('back', 'mira', { answers: 'ask' }), position: 5 };
  state.turns.heard = { ...turn('heard', 'mira', { answers: 'ask' }), position: 6 };
  state.turns.woke = { ...turn('woke', 'mira'), position: 7 };
  state.messages.push(
    said('back', 'gathr', 'The task you gave is settled:\n- #1 Rowan, Task 1: done\nRead replies.'),
    said('back', 'assistant', 'Rowan is done'),
    said('heard', 'gathr', 'The person answered:\n- #5 Which notice?: B: two sections\nContinue.'),
    said('heard', 'assistant', 'B it is'),
    said('woke', 'gathr', 'Your wait on CI on #12 ended'),
    said('woke', 'assistant', 'CI passed'),
  );
  const { items } = view(state);
  expect(events(items)).toEqual([
    { kind: 'question', who: 'Mira asked you:', text: 'Which notice?', more: 'Question #5', task: 5 },
    { kind: 'question', who: 'Mira asked you:', text: 'Task 6', more: 'Question #6', task: 6 },
    {
      kind: 'task',
      who: 'Rowan completed:',
      text: 'Task 1',
      more: 'Task #1. Approved at head 9a41c07',
      task: 1,
    },
    {
      kind: 'answer',
      who: 'You answered Mira:',
      text: 'B: two sections',
      more: 'Question #5: Which notice?',
      task: 5,
    },
    { kind: 'question', who: 'Mira asked you:', text: 'Ship it?', more: 'Question #9. Two halves', task: 9 },
    { kind: 'note', text: 'Your wait on CI on #12 ended' },
  ]);
  // Each line sits where it happened: before the reply it woke, a question after the turn that asked it.
  expect(ids(items).slice(-7)).toEqual([
    'back-gathr',
    'a-back',
    'heard-gathr',
    'a-heard',
    'asked-heard',
    'woke-gathr',
    'a-woke',
  ]);
  expect(items.filter((m) => custom(m).events).every((m) => m.role === 'system')).toBe(true);
});

test('a speaker is named when it changes, not on each of its blocks in a row', () => {
  const state = round();
  state.turns.again = { ...turn('again', 'kit', { task: 2 }), position: 5 };
  state.turns.other = { ...turn('other', 'kit', { task: 3 }), position: 6 };
  state.tasks.push(task(3, 'kit', 'ask'));
  state.messages.push(said('again', 'assistant', 'One more thing'), said('other', 'assistant', 'Next task'));
  const { items } = view(state);
  const heads = Object.fromEntries(items.map((m) => [m.id, custom(m).head]));
  expect(heads).toMatchObject({
    'a-ask': true,
    'a-rowan': true,
    'a-kit': true,
    'a-again': false,
    // The same agent on another task says which.
    'a-other': true,
  });
});

test('a search hit opens at its message: the text in its block, or the block for what isn’t drawn', () => {
  const state = round();
  state.messages.push(
    said('rowan', 'assistant', 'Same words'),
    tool('rowan', 'r1', 'Bash'),
    said('rowan', 'assistant', 'Same words'),
  );
  const { find } = view(state);
  const [first, , last] = state.messages.slice(-3);
  // A mid-work text is in the row; the same words as the message are the second passage.
  expect(find(first.id)).toEqual({ id: 'a-rowan', text: 'Same words', nth: 0 });
  expect(find(last.id)).toEqual({ id: 'a-rowan', text: 'Same words', nth: 1 });
  // Gathr's message that gave the task isn't drawn: the member's block stands for it.
  const gave = state.messages.find((m) => m.role === 'gathr' && m.turn === 'rowan') as Message;
  expect(find(gave.id)).toEqual({ id: 'a-rowan' });
  expect(find('gone')).toBeUndefined();
});
