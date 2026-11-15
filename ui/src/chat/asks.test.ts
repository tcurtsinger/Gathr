// Every ask has a size (0061): where its size line sits in the thread, what it reads, and the line
// Gathr says when a repair's work is in.
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

const view = (state: ChatState) => toThread(state, agents, [], new Map()).items;
const custom = (m: ThreadMessageLike | undefined) => (m?.metadata?.custom ?? {}) as Custom;
const ids = (items: ThreadMessageLike[]) => items.map((m) => m.id);
const line = (items: ThreadMessageLike[], id: string) => custom(items.find((m) => m.id === id)).events;
const sizes = (items: ThreadMessageLike[]) => ids(items).filter((id) => id?.startsWith('size-'));

test("a sized ask's line sits after the lead's turn that sized it, and reads its name and size", () => {
  const question = task(2, { kind: 'person', owner: null, given_in: 'hb', what: 'Which export?' });
  const state = chat({
    tasks: [question],
    asks: [ask('m1', { size: 'repair', by: 'mira', sized_in: 'hb' })],
  });
  const items = view(state);
  expect(ids(items)).toEqual(['m1', 'a-ask', 'g1', 'a-hb', 'size-m1', 'asked-hb']);
  expect(line(items, 'size-m1')).toEqual([{ kind: 'fix', who: 'Mira:', text: 'Fix', ask: 'm1' }]);

  const resized = chat({
    tasks: [question],
    asks: [ask('m1', { size: 'new', by: 'person', sized_in: 'hb' })],
  });
  const again = view(resized);
  expect(ids(again)).toEqual(['m1', 'a-ask', 'g1', 'a-hb', 'size-m1', 'asked-hb']);
  expect(line(again, 'size-m1')).toEqual([{ kind: 'plan', who: 'You:', text: 'Plan', ask: 'm1' }]);
});

test('a message steered into the turn that sized it has its line below it', () => {
  const items = view(
    steered({ asks: [ask('m0'), ask('m2', { size: 'repair', by: 'mira', sized_in: 'lead' })] }),
  );
  expect(ids(items)).toEqual(['m0', 'a-lead', 'm2', 'a-lead-1', 'size-m2']);
  expect(line(items, 'size-m2')).toEqual([{ kind: 'fix', who: 'Mira:', text: 'Fix', ask: 'm2' }]);
});

test('a split ask whose message moved into a later turn has its line below it', () => {
  // `m2` was steered into `lead` and split off there, but the engine read it only after `lead` ended.
  clock = 0;
  const items = view({
    ...empty,
    members,
    turns: { lead: { ...turn('lead'), position: 0 }, later: { ...turn('later'), position: 1 } },
    messages: [
      msg('m1', 'lead', 'user', 'Rename the export menu'),
      msg('r1', 'lead', 'assistant', 'On it'),
      msg('m2', 'later', 'user', 'Add tags to notes'),
      msg('r2', 'later', 'assistant', 'That one is its own ask'),
    ],
    asks: [ask('m1'), ask('m2', { size: 'new', by: 'mira', sized_in: 'lead' })],
  });
  expect(ids(items)).toEqual(['m1', 'a-lead', 'm2', 'a-later', 'size-m2']);
  expect(line(items, 'size-m2')).toEqual([{ kind: 'plan', who: 'Mira:', text: 'Plan', ask: 'm2' }]);
});

test('an unsized ask with no task draws no line', () => {
  // Another ask's task, whose message isn't loaded, draws none either.
  const items = view(chat({ tasks: [task(1, { ask: 'm9' })], asks: [ask('m9'), ask('m1')] }));
  expect(ids(items)).toEqual(['m1', 'a-ask', 'g1', 'a-hb']);
  expect(sizes(items)).toEqual([]);
});

test("an unsized ask with a task reads Plan after its message's turn", () => {
  // Any state counts: this one is done.
  const items = view(chat({ tasks: [task(1, { ask: 'm1', state: 'done' })], asks: [ask('m1')] }));
  expect(ids(items)).toEqual(['m1', 'a-ask', 'size-m1', 'g1', 'a-hb']);
  expect(line(items, 'size-m1')).toEqual([{ kind: 'plan', text: 'Plan', ask: 'm1' }]);

  // Steered into a running turn: after the block that follows it, not the turn's first block above it.
  const into = view(
    steered({ tasks: [task(1, { ask: 'm2', given_in: 'lead' })], asks: [ask('m0'), ask('m2')] }),
  );
  expect(ids(into)).toEqual(['m0', 'a-lead', 'm2', 'a-lead-1', 'size-m2']);

  // A question to the person is a task of the ask too; the line comes before the questions.
  const question = task(3, { kind: 'person', owner: null, ask: 'm1', what: 'Which export?' });
  const asked = view(chat({ tasks: [question], asks: [ask('m1')] }));
  expect(ids(asked)).toEqual(['m1', 'a-ask', 'size-m1', 'asked-ask', 'g1', 'a-hb']);
  expect(line(asked, 'size-m1')).toEqual([{ kind: 'plan', text: 'Plan', ask: 'm1' }]);
});

test('a live tasks event that brings the first task makes the line appear', () => {
  const before = chat({ asks: [ask('m1')] });
  expect(sizes(view(before))).toEqual([]);
  const after = apply(before, {
    type: 'tasks',
    chat: 'c',
    tasks: [task(1, { ask: 'm1' })],
    waits: [],
    asks: [ask('m1')],
  });
  const items = view(after);
  expect(sizes(items)).toEqual(['size-m1']);
  expect(line(items, 'size-m1')).toEqual([{ kind: 'plan', text: 'Plan', ask: 'm1' }]);
});

test('an ask the person sized that the lead never sized', () => {
  const items = view(chat({ asks: [ask('m1', { size: 'repair', by: 'person', sized_in: null })] }));
  expect(ids(items)).toEqual(['m1', 'a-ask', 'size-m1', 'g1', 'a-hb']);
  expect(line(items, 'size-m1')).toEqual([{ kind: 'fix', who: 'You:', text: 'Fix', ask: 'm1' }]);
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
