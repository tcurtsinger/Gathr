// A held turn's caught change (0063 I2b.2): Gathr's line in that turn, after its blocks, opening to the files.
import type { ThreadMessageLike } from '@assistant-ui/react';
import { expect, test } from 'vitest';
import type { Agent, Message, Turn } from '../api';
import { type Custom, toThread } from './messages';
import { apply, type ChatState, empty } from './state';

const agents: Agent[] = ['Mira', 'Ann'].map((name) => ({
  id: name.toLowerCase(),
  name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto',
}));
const members = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' as const },
  { agent: 'ann', name: 'Ann', role: 'member' as const },
];
const turn = (id: string, position: number): Turn => ({
  id,
  agent: 'mira',
  status: 'done',
  why: null,
  ref: id,
  position,
});
let clock = 0;
const msg = (id: string, role: Message['role'], body: Message['body'], kind = 'text'): Message => ({
  id,
  turn: 'held',
  time: new Date(Date.UTC(2026, 10, 15, 9, 0, clock++)).toISOString(),
  role,
  kind,
  body,
});

const said = [
  'Mira changed 3 files before the plan was approved',
  'new.txt',
  'src/notes.rs',
  'src/tags.rs',
].join('\n');

/** The person asks Mira (`m1`); her held turn `held` runs a command, replies, and Gathr's line `g1` ends it. */
function chat(more: Partial<ChatState> = {}): ChatState {
  clock = 0;
  return {
    ...empty,
    members,
    turns: { held: turn('held', 0) },
    messages: [
      msg('m1', 'user', 'Add tags to notes'),
      msg('t1', 'assistant', { tool: 'Bash', status: 'ok' }, 'tool'),
      msg('r1', 'assistant', 'Tags need a plan first'),
      msg('g1', 'gathr', said),
    ],
    ...more,
  };
}

const view = (state: ChatState) => toThread(state, agents, [], new Map()).items;
const ids = (items: ThreadMessageLike[]) => items.map((m) => m.id);
const line = (items: ThreadMessageLike[], id: string) =>
  ((items.find((m) => m.id === id)?.metadata?.custom ?? {}) as Custom).events;

test("a caught change draws as one Changed line after its turn's blocks, opening to the files", () => {
  const items = view(chat({ caught: ['g1'] }));
  expect(ids(items)).toEqual(['m1', 'a-held', 'g1']);
  expect(line(items, 'g1')).toEqual([
    {
      kind: 'changed',
      text: 'Mira changed 3 files before the plan was approved',
      more: 'new.txt\nsrc/notes.rs\nsrc/tags.rs',
    },
  ]);
});

test('without its id in caught, the same message draws as a note with all its lines', () => {
  const items = view(chat());
  expect(ids(items)).toEqual(['m1', 'a-held', 'g1']);
  expect(line(items, 'g1')).toEqual([{ kind: 'note', text: said }]);

  // Live: the tasks event that carries the line's id makes it the Changed line.
  const after = apply(chat(), { type: 'tasks', chat: 'c', tasks: [], waits: [], asks: [], caught: ['g1'] });
  expect(line(view(after), 'g1')?.map((e) => e.kind)).toEqual(['changed']);
});
