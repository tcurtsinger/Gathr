// Every ask has a size (0061): which message it sits on, what it says, and the line Gathr says when a repair's
// work is in.
import type { ThreadMessageLike } from '@assistant-ui/react';
import { expect, test } from 'vitest';
import type { Agent, Ask, Message, Task, Turn } from '../api';
import { type Custom, toThread } from './messages';
import { apply, type ChatState, empty } from './state';

const agents: Agent[] = ['Mira', 'Rowan', 'Kit'].map((name) => ({
  id: name.toLowerCase(),
  name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto',
  face_seed: 'Made-up',
  gender: null,
  skin: null,
  background: null,
}));
const members = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' as const },
  { agent: 'rowan', name: 'Rowan', role: 'member' as const },
  { agent: 'kit', name: 'Kit', role: 'member' as const },
];

let clock = 0;
const at = () => new Date(Date.UTC(2026, 10, 15, 9, 0, clock++)).toISOString();
const turn = (id: string, agent = 'mira', more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  created: at(),
  ...more,
});
const msg = (id: string, turnId: string | null, role: Message['role'], body: string): Message => ({
  id,
  turn: turnId,
  time: at(),
  role,
  kind: 'text',
  body,
});
const task = (number: number, more: Partial<Task> = {}): Task => ({
  number,
  chat: 'c',
  giver: 'mira',
  owner: 'rowan',
  given_in: 'ask',
  kind: 'work',
  what: `Task ${number}`,
  state: 'working',
  reason: null,
  after: [],
  created_us: 0,
  ...more,
});
const ask = (id: string, more: Partial<Ask> = {}): Ask => ({
  id,
  title: `Ask ${id}`,
  size: null,
  by: null,
  sized_in: null,
  held: false,
  ...more,
});

/** The person asks Mira (`m1`, turn `ask`) and Mira answers; later Mira's hear-back turn `hb` replies. */
function chat(more: Partial<ChatState> = {}): ChatState {
  clock = 0;
  const turns = [turn('ask'), turn('hb', 'mira', { answers: 'ask' })];
  return {
    ...empty,
    members,
    turns: Object.fromEntries(turns.map((t, position) => [t.id, { ...t, position }])),
    messages: [
      msg('m1', 'ask', 'user', 'The export button does nothing'),
      msg('r1', 'ask', 'assistant', 'Looking into it'),
      msg('g1', 'hb', 'gathr', 'Where the task you gave stands:'),
      msg('r2', 'hb', 'assistant', 'Rowan is on it'),
    ],
    ...more,
  };
}

/** Mira's turn `lead`, opened by `m0`, with `m2` sent into it while it ran, and her reply after each. */
function steered(more: Partial<ChatState> = {}): ChatState {
  clock = 0;
  return {
    ...empty,
    members,
    turns: { lead: { ...turn('lead'), position: 0 } },
    messages: [
      msg('m0', 'lead', 'user', 'Rename the export menu'),
      msg('r0', 'lead', 'assistant', 'On it'),
      msg('m2', 'lead', 'user', 'Add tags to notes'),
      msg('r3', 'lead', 'assistant', 'That one is its own ask'),
    ],
    ...more,
  };
}

// Every block, in its thread or not: what each block is, not which threads are open (0075).
const view = (state: ChatState) => toThread(state, agents, [], new Map()).all;
const custom = (m: ThreadMessageLike | undefined) => (m?.metadata?.custom ?? {}) as Custom;
const ids = (items: ThreadMessageLike[]) => items.map((m) => m.id);
const line = (items: ThreadMessageLike[], id: string) => custom(items.find((m) => m.id === id)).events;
/** Each message's size, by the message's id: none drawn for the rest. */
const sizes = (items: ThreadMessageLike[]) =>
  Object.fromEntries(items.flatMap((m) => (custom(m).size ? [[m.id, custom(m).size]] : [])));
const mira = { agent: 'mira', name: 'Mira' };

test('a sized ask’s size sits on its own message, with who sized it, and on no line of its own', () => {
  const question = task(2, { kind: 'person', owner: null, given_in: 'hb', what: 'Which export?' });
  const state = chat({
    tasks: [question],
    asks: [ask('m1', { size: 'repair', by: 'mira', sized_in: 'hb' })],
  });
  const items = view(state);
  expect(ids(items)).toEqual(['m1', 'a-ask', 'g1', 'a-hb']);
  expect(sizes(items)).toEqual({ m1: { ask: 'm1', size: 'repair', by: mira } });

  const resized = chat({
    tasks: [question],
    asks: [ask('m1', { size: 'new', by: 'person', sized_in: 'hb' })],
  });
  expect(sizes(view(resized))).toEqual({ m1: { ask: 'm1', size: 'new', yours: true } });
});

test('a message steered into a running turn and split off carries its own size', () => {
  const items = view(
    steered({ asks: [ask('m0'), ask('m2', { size: 'repair', by: 'mira', sized_in: 'lead' })] }),
  );
  expect(ids(items)).toEqual(['m0', 'a-lead', 'm2', 'a-lead-1']);
  expect(sizes(items)).toEqual({ m2: { ask: 'm2', size: 'repair', by: mira } });
});

test('an unsized ask not held, with no task, still shows none', () => {
  // Another ask's task, whose message isn't loaded, shows none either.
  const items = view(
    chat({ tasks: [task(1, { ask: 'm9' })], asks: [ask('m9'), ask('m1', { held: false })] }),
  );
  expect(ids(items)).toEqual(['m1', 'a-ask', 'g1', 'a-hb']);
  expect(sizes(items)).toEqual({});
});

test('an unsized ask Gathr held, or with a task, reads Plan, live too', () => {
  expect(sizes(view(chat({ asks: [ask('m1', { held: true })] })))).toEqual({
    m1: { ask: 'm1', size: 'new' },
  });
  // Any state counts: this one is done. A question to the person is a task of the ask too.
  const done = chat({ tasks: [task(1, { ask: 'm1', state: 'done' })], asks: [ask('m1')] });
  expect(sizes(view(done))).toEqual({ m1: { ask: 'm1', size: 'new' } });
  const question = task(3, { kind: 'person', owner: null, ask: 'm1', what: 'Which export?' });
  expect(sizes(view(chat({ tasks: [question], asks: [ask('m1')] })))).toEqual({
    m1: { ask: 'm1', size: 'new' },
  });

  // Live: the tasks event that says Gathr held it, or that brings the first task, brings the size.
  const before = chat({ asks: [ask('m1')] });
  expect(sizes(view(before))).toEqual({});
  for (const [tasks, asks] of [
    [[], [ask('m1', { held: true })]],
    [[task(1, { ask: 'm1' })], [ask('m1')]],
  ] as const) {
    const after = apply(before, { type: 'tasks', chat: 'c', tasks: [...tasks], waits: [], asks: [...asks] });
    expect(sizes(view(after))).toEqual({ m1: { ask: 'm1', size: 'new' } });
  }
});

test('an ask the person sized that the lead never sized', () => {
  const items = view(chat({ asks: [ask('m1', { size: 'repair', by: 'person', sized_in: null })] }));
  expect(sizes(items)).toEqual({ m1: { ask: 'm1', size: 'repair', yours: true } });
});

test("a repair's Fixed line opens to its tasks' PRs and proof", () => {
  const state = chat();
  const said = [
    'Fixed: The export button does nothing',
    '#1 Find the bug: PR #41. Proof: the CI run on the PR',
    '#2 Check the import: src/export.rs. Proof: the export test passes',
  ].join('\n');
  state.messages.push(msg('fx', null, 'gathr', said));
  const items = view(state);
  expect(ids(items).at(-1)).toBe('fx');
  expect(line(items, 'fx')).toEqual([
    {
      kind: 'task',
      who: 'Fixed:',
      text: 'The export button does nothing',
      more: [
        '#1 Find the bug: PR #41. Proof: the CI run on the PR',
        '#2 Check the import: src/export.rs. Proof: the export test passes',
      ].join('\n'),
    },
  ]);
});
