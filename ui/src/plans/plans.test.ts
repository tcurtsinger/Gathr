// A plan in the timeline (0062 H5.1, H5.2): the lead's version as its own block with the chip, Gathr's plan
// lines as kind `version`, and Summary's round without the plan's check.
import type { ThreadMessageLike } from '@assistant-ui/react';
import { expect, test } from 'vitest';
import type { Agent, Message, PlanSummary, Task, Turn } from '../api';
import { type Custom, toThread } from '../chat/messages';
import { currentRound } from '../chat/plan';
import { type ChatState, empty } from '../chat/state';

const agents: Agent[] = ['Mira', 'Ann', 'Bo'].map((name) => ({
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
  { agent: 'bo', name: 'Bo', role: 'reviewer' as const },
];
const t0 = Date.UTC(2026, 10, 15, 9, 0, 0);
const iso = (s: number) => new Date(t0 + s * 1000).toISOString();
const turn = (id: string, agent: string, position: number, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  created: iso(position * 100),
  position,
  ...more,
});
const message = (
  id: string,
  turn: string | null,
  role: Message['role'],
  s: number,
  body: string,
): Message => ({
  id,
  turn,
  time: iso(s),
  role,
  kind: 'text',
  body,
});
const call = (id: string, turn: string, tool: string, s: number): Message => ({
  id,
  turn,
  time: iso(s),
  role: 'assistant',
  kind: 'tool',
  body: { tool, status: 'ok' },
});
const task = (number: number, owner: string, more: Partial<Task> = {}): Task => ({
  number,
  chat: 'c',
  giver: 'mira',
  owner,
  given_in: 'ask',
  round: 'ask',
  kind: 'work',
  what: `Task ${number}`,
  state: 'ready',
  reason: null,
  after: [],
  created_us: 0,
  ...more,
});
const body = (n: number) =>
  `Plan: Tags on notes (version ${n}, 9 parts)\nTags for each note.\n\n1. Add the table (Ann)`;

/**
 * The person asks Mira for tags; Mira reads, gives Ann a task, writes version 1 and says more after it.
 * Bo agrees; the person sends it back; Mira writes version 2, Bo gives it a thumbs-up, and it is approved.
 */
function planned(): ChatState {
  const plan: PlanSummary = {
    id: 3,
    ask: 'm1',
    title: 'Tags on notes',
    lead: 'mira',
    state: 'approved',
    versions: [
      {
        id: 11,
        n: 1,
        message: 'v1',
        parts: 9,
        check_line: 'c1',
        sent: iso(160),
        decided: 'sent_back',
        decided_line: 'sb',
      },
      {
        id: 12,
        n: 2,
        message: 'v2',
        parts: 9,
        check_line: null,
        sent: iso(400),
        decided: 'approved',
        decided_line: 'ap',
      },
    ],
  };
  return {
    ...empty,
    members,
    turns: {
      ask: turn('ask', 'mira', 0),
      check: turn('check', 'bo', 1, { task: 2, answers: 'ask' }),
      back: turn('back', 'mira', 2, { answers: 'ask' }),
    },
    messages: [
      message('m1', 'ask', 'user', 0, 'Add tags to notes'),
      message('r1', 'ask', 'assistant', 10, 'Reading the notes code first.'),
      call('g1', 'ask', 'mcp__gathr__give_task', 20),
      call('p1', 'ask', 'mcp__gathr__plan', 30),
      message('v1', 'ask', 'assistant', 31, body(1)),
      message('r2', 'ask', 'assistant', 40, 'Bo checks it next.'),
      message('b1', 'check', 'gathr', 100, 'Task #2 from Mira: Check plan v1: Tags on notes'),
      message('c1', null, 'gathr', 160, 'Bo agrees with version 1 of the plan "Tags on notes".'),
      message(
        'sb',
        'back',
        'gathr',
        200,
        'The person sent back version 1 of your plan "Tags on notes" (plan #3):\nSplit part 2\nWrite the next version with plan.',
      ),
      message('v2', 'back', 'assistant', 210, body(2)),
      message('ap', null, 'gathr', 500, 'Approved: version 2 of the plan "Tags on notes", 9 tasks.'),
    ],
    tasks: [
      task(1, 'ann', { what: 'Read the notes schema' }),
      task(2, 'bo', {
        kind: 'review',
        what: 'Check plan v1: Tags on notes',
        checks: 11,
        state: 'done',
        report: { how: 'done', note: null, see: null, at_us: (t0 + 150_000) * 1000 },
      }),
    ],
    asks: [{ id: 'm1', title: 'Add tags to notes', size: 'new', by: 'mira', sized_in: 'ask' }],
    plans: [plan],
    reactions: [{ message: 'v2', agent: 'bo', time: iso(220) }],
  };
}

const view = (state: ChatState) => toThread(state, agents, [], new Map()).items;
const custom = (m: ThreadMessageLike | undefined) => (m?.metadata?.custom ?? {}) as Custom;
const ids = (items: ThreadMessageLike[]) => items.map((m) => m.id);
const text = (m: ThreadMessageLike | undefined) =>
  ((m?.content ?? []) as readonly { text?: string }[]).map((p) => p.text).join(' | ');

test("a version is its own block after the lead's earlier text, headed by the lead, with its chip and thumbs-up", () => {
  const items = view(planned());
  const order = ids(items);
  expect(order.indexOf('v1')).toBeGreaterThan(order.indexOf('a-ask'));
  const earlier = custom(items.find((m) => m.id === 'a-ask')).work?.steps;
  expect(earlier).toContainEqual(expect.objectContaining({ say: 'Reading the notes code first.' }));
  const second = items.find((m) => m.id === 'v2');
  expect(second?.role).toBe('assistant');
  expect(custom(second)).toMatchObject({
    agent: 'Mira',
    head: true,
    plan: { plan: 3, version: 2, parts: 9 },
    agreed: ['Bo'],
  });
  // Its words are the tab's, not the timeline's.
  expect(text(second)).toBe('');
});

test("the lead's text after the plan call reads in a block after the chip, not before it", () => {
  const order = ids(view(planned()));
  expect(order.slice(order.indexOf('a-ask'), order.indexOf('a-ask-1') + 1)).toEqual([
    'a-ask',
    'size-m1',
    'v1',
    'a-ask-1',
  ]);
  expect(text(view(planned()).find((m) => m.id === 'a-ask-1'))).toBe('Bo checks it next.');
});

test("Gathr's plan lines are kind version, each with its plan; the size line keeps kind plan", () => {
  const items = view(planned());
  const lines = items.flatMap((m) => custom(m).events ?? []);
  expect(lines).toEqual([
    { kind: 'plan', who: 'Mira:', text: 'Plan', ask: 'm1' },
    {
      kind: 'version',
      who: 'Plan v1:',
      text: 'Bo agrees',
      more: 'Tags on notes',
      plan: { id: 3, version: 1 },
    },
    {
      kind: 'version',
      who: 'Plan v1:',
      text: 'Sent back',
      more: 'Split part 2',
      plan: { id: 3, version: 1 },
    },
    {
      kind: 'version',
      who: 'Plan v2:',
      text: 'Approved',
      more: 'Tags on notes',
      plan: { id: 3, version: 2 },
    },
  ]);
});

test('a check line says the outcome the version reached the person with', () => {
  const findings = planned();
  findings.plans[0].versions[0].sent = null;
  findings.tasks[1] = {
    ...findings.tasks[1],
    state: 'blocked',
    report: { how: 'left', note: null, see: null, at_us: (t0 + 150_000) * 1000 },
    notes: [
      { kind: 'wrong', file: 'src/notes.rs', says: 'Made-up wrong fact' },
      { kind: 'unasked', file: null, says: 'Made-up extra' },
      { kind: 'suggestion', file: null, says: 'Made-up idea' },
    ],
  };
  const late = planned();
  // Its check agreed only after Gathr sent the version on, late.
  late.tasks[1] = {
    ...late.tasks[1],
    report: { how: 'done', note: null, see: null, at_us: (t0 + 900_000) * 1000 },
  };
  const failed = planned();
  failed.tasks[1] = { ...failed.tasks[1], state: 'failed', report: null };
  const said = (state: ChatState) =>
    view(state)
      .flatMap((m) => custom(m).events ?? [])
      .find((e) => e.kind === 'version')?.text;
  expect([findings, late, failed].map(said)).toEqual(['2 findings', "Bo didn't finish", "Bo didn't finish"]);
});

test("a hear-back says no line for a plan's check, whose plan line says how it ended, and keeps the others", () => {
  const state = planned();
  state.plans[0].versions[0].sent = null;
  state.tasks[0] = {
    ...state.tasks[0],
    state: 'done',
    report: { how: 'done', note: null, see: null, at_us: 0 },
  };
  state.tasks[1] = {
    ...state.tasks[1],
    state: 'blocked',
    report: { how: 'left', note: 'Two gaps', see: null, at_us: (t0 + 150_000) * 1000 },
    notes: [{ kind: 'wrong', file: null, says: 'Made-up wrong fact' }],
  };
  // Mira hears of both before the person sends version 1 back.
  state.turns.hear = turn('hear', 'mira', 1.5, { task: 9 });
  state.messages.splice(
    8,
    0,
    message(
      'h1',
      'hear',
      'gathr',
      170,
      'The 2 tasks you gave are settled:\n- #1 Ann, Read the notes schema: done\n' +
        '- #2 Bo, Check plan v1: Tags on notes: left: Two gaps. Findings: note 1, wrong: Made-up wrong fact. ' +
        "Answer each in the next version's answers.\nRead replies with read_chat, then continue.",
    ),
  );
  const lines = view(state).flatMap((m) => custom(m).events ?? []);
  expect(
    lines.filter((e) => e.kind === 'task' || e.kind === 'version').map((e) => `${e.who} ${e.text}`),
  ).toEqual([
    'Plan v1: 1 finding',
    'Ann completed: Read the notes schema',
    'Plan v1: Sent back',
    'Plan v2: Approved',
  ]);
});

test('a give_task call in the same turn as the plan keeps its label', () => {
  const work = custom(view(planned()).find((m) => m.id === 'a-ask')).work;
  expect(work?.steps.find((s) => s.id === 'g1')).toMatchObject({ label: 'Gave Ann a task', task: 1 });
});

test("Summary's round leaves out the plan's check; a round of a done check alone is no round", () => {
  const state = planned();
  expect(currentRound(state)?.steps.map((s) => s.task)).toEqual([1]);
  const checkOnly = { ...state, tasks: [state.tasks[1]] };
  expect(currentRound(checkOnly)).toBeUndefined();
});
