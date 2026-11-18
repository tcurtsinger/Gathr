// A made-up round of a team chat, for the timeline's tests: the person asks Mira, Mira hands Rowan and Kit a task
// each, and what the timeline makes of it.
import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Agent, Message, Task, ToolBody, Turn } from '../api';
import { type Custom, toThread } from './messages';
import { type ChatState, empty } from './state';

export const people = ['Mira', 'Rowan', 'Kit', 'Lane', 'Sol'];
export const agents: Agent[] = people.map((name) => ({
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
export const members = people.map((name, i) => ({
  agent: name.toLowerCase(),
  name,
  role: i === 0 ? ('supervisor' as const) : i === 4 ? ('reviewer' as const) : ('member' as const),
}));

let clock = 0;
export const at = () => new Date(Date.UTC(2026, 10, 8, 9, 0, clock++)).toISOString();
export const turn = (id: string, agent: string, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  created: at(),
  ...more,
});
export const said = (turnId: string, role: Message['role'], text: string): Message => ({
  id: `${turnId}-${role}-${clock}`,
  turn: turnId,
  time: at(),
  role,
  kind: 'text',
  body: text,
});
export const tool = (
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
export const task = (number: number, owner: string, given_in: string, more: Partial<Task> = {}): Task => ({
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
export function round(): ChatState {
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

export const view = (state: ChatState) => toThread(state, agents, [], new Map());
export const custom = (m: ThreadMessageLike | undefined) => (m?.metadata?.custom ?? {}) as Custom;
export const ids = (all: ThreadMessageLike[]) =>
  all.map((m) => (m.id ?? '').replace(/-(user|gathr)-\d+$/, '-$1'));
export const byId = (all: ThreadMessageLike[], id: string) => all.find((m) => m.id === id);
export const text = (m: ThreadMessageLike | undefined) =>
  ((m?.content ?? []) as readonly { text?: string }[]).map((p) => p.text).join(' | ');
export const events = (all: ThreadMessageLike[]) => all.flatMap((m) => custom(m).events ?? []);
