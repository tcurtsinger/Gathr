// The conversation map's ticks (0039): which blocks of the timeline get one, what each says, and how
// earlier days fold once the rail can't hold a tick for each.
import { expect, test } from 'vitest';
import type { Message, Task, Turn } from '../api';
import { mapEntries, plain, type Tick, ticksOf } from './map';
import { toThread } from './messages';
import { type ChatState, empty } from './state';

// Local times, so what a tick says doesn't depend on the machine's timezone.
const at = (day: number, hour: number, minute = 0) => new Date(2026, 10, day, hour, minute).toISOString();
const now = new Date(2026, 10, 11, 18, 0);

const turn = (id: string, agent: string | null, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  ...more,
});
const said = (id: string, turnId: string, role: Message['role'], body: string, more: Partial<Message> = {}) =>
  ({ id, turn: turnId, time: at(11, 16, 4), role, kind: 'text', body, ...more }) as Message;
const tool = (id: string, turnId: string): Message => ({
  id,
  turn: turnId,
  time: at(11, 16, 4),
  role: 'assistant',
  kind: 'tool',
  body: { tool: 'Read', status: 'ok' },
});
const state = (turns: Turn[], messages: Message[], more: Partial<ChatState> = {}): ChatState => ({
  ...empty,
  messages,
  turns: Object.fromEntries(turns.map((t, position) => [t.id, { ...t, position }])),
  ...more,
});
const ticks = (s: ChatState) => ticksOf(toThread(s, [], [], new Map()).items);
const brief = (t: Tick) => `${t.kind} ${t.who}: ${t.text}`;

const team = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' as const },
  { agent: 'rowan', name: 'Rowan', role: 'member' as const },
];
const task: Task = {
  number: 1,
  chat: 'c',
  giver: 'mira',
  owner: 'rowan',
  given_in: 'ask',
  kind: 'work',
  what: 'Fix the cursor',
  state: 'done',
  reason: null,
  after: [],
  created_us: 0,
};

test("the person's messages and the lead's replies to them get ticks; nothing else does", () => {
  const round = state(
    [
      turn('ask', 'mira'),
      turn('rowan', 'rowan', { task: 1, answers: 'ask' }),
      // Woken by Rowan's message: team work, not a reply to the person.
      turn('woke', 'mira', { answers: 'rowan' }),
      // The hear-back: the lead reports to the person.
      turn('back', 'mira', { answers: 'ask' }),
      turn('direct', 'rowan'),
    ],
    [
      said('u1', 'ask', 'user', 'Fix the reconnect'),
      tool('x1', 'ask'),
      said('l1', 'ask', 'assistant', 'Rowan, over to you'),
      said('g1', 'rowan', 'gathr', 'Task #1 from Mira: Fix the cursor'),
      said('r1', 'rowan', 'assistant', 'Cursor fixed'),
      said('r2', 'rowan', 'assistant', 'Which branch?', { to: 'mira' }),
      said('w1', 'woke', 'assistant', 'Noted'),
      said('g2', 'back', 'gathr', 'A made-up wake-up'),
      said('l2', 'back', 'assistant', 'The cursor is fixed'),
      said('u2', 'direct', 'user', '@Rowan which branch was it?'),
      said('r3', 'direct', 'assistant', 'The reconnect one'),
    ],
    { members: team, tasks: [task] },
  );
  expect(ticks(round).map(brief)).toEqual([
    'you You: Fix the reconnect',
    'lead Mira: Rowan, over to you',
    'lead Mira: The cursor is fixed',
    'you You: @Rowan which branch was it?',
  ]);
});

test("in a chat with one agent its replies are the short ticks, one for each turn's last words", () => {
  const solo = state(
    [turn('a', 'mira'), turn('b', 'mira'), turn('c', 'mira')],
    [
      said('u1', 'a', 'user', 'Made-up ask'),
      said('a1', 'a', 'assistant', 'Starting'),
      // Sent into the running turn: it splits the turn, and only the turn's last block is its reply.
      said('u2', 'a', 'user', 'And the tests'),
      said('a2', 'a', 'assistant', 'Both done'),
      said('u3', 'b', 'user', 'Read the file'),
      // A turn that ended on a tool call said nothing to tick.
      tool('x', 'b'),
      said('u4', 'c', 'user', 'Thanks'),
      said('a3', 'c', 'assistant', 'Any time'),
    ],
    { members: [team[0]] },
  );
  expect(ticks(solo).map(brief)).toEqual([
    'you You: Made-up ask',
    'you You: And the tests',
    'lead Mira: Both done',
    'you You: Read the file',
    'you You: Thanks',
    'lead Mira: Any time',
  ]);
});

test("a tick's card says who and when, and the message's first lines without their Markdown", () => {
  const reply = [
    '## Plan',
    '- **Keep** `main_branch` green, see [the notes](https://example.com/notes)',
    '> *Then* ship_it ~~later~~',
  ].join('\n');
  const solo = state(
    [turn('a', 'mira')],
    [said('u1', 'a', 'user', 'Made-up ask'), said('a1', 'a', 'assistant', reply, { time: at(11, 16, 5) })],
    { members: [team[0]] },
  );
  const entries = mapEntries(ticks(solo), 50, now);
  expect(entries.map((e) => [e.kind, e.title, e.preview])).toEqual([
    ['you', 'You · 16:04', 'Made-up ask'],
    ['lead', 'Mira · 16:05', 'Plan Keep main_branch green, see the notes Then ship_it later'],
  ]);
});

test('plain text keeps code as written and a table as its words', () => {
  expect(plain('Run `npm run *.test` now\n\n| Step | Time |\n|---|---|\n| Build | 2 s |')).toBe(
    'Run npm run *.test now Step Time Build 2 s',
  );
  expect(plain('```sh\ngit status\n```\n1. First\n2) Second')).toBe('git status First Second');
});

/** A day of the chat: asks from the person, each with the lead's reply. */
const day = (date: number, asks: number): Tick[] =>
  Array.from({ length: asks }, (_, n): Tick[] => [
    { id: `u${date}-${n}`, kind: 'you', who: 'You', time: at(date, 9, n), text: `Ask ${n}` },
    { id: `l${date}-${n}`, kind: 'lead', who: 'Mira', time: at(date, 9, n), text: `Reply ${n}` },
  ]).flat();
// Monday 2 November to Wednesday 11 November 2026, which is today.
const chat = [...day(2, 2), ...day(9, 14), ...day(10, 3), ...day(11, 4)];
const shape = (most: number) => mapEntries(chat, most, now).map((e) => `${e.kind} ${e.title}`);

test('with room for every tick, none folds', () => {
  const entries = mapEntries(chat, 46, now);
  expect(entries).toHaveLength(46);
  expect(entries.some((e) => e.kind === 'day')).toBe(false);
});

test('short of room, the oldest days fold first, each to one tick that opens its first message', () => {
  // One too many: only the oldest day folds. It is over a week old, so it goes by its date.
  const one = mapEntries(chat, 45, now);
  expect(one).toHaveLength(43);
  expect(one[0]).toMatchObject({ kind: 'day', id: 'u2-0', preview: 'Jumps to the start of that day' });
  expect(one[0].title).toMatch(/^(2 Nov|Nov 2) · 2 messages from you$/);
  expect(one[1]).toMatchObject({ kind: 'you', id: 'u9-0' });

  // Less room: the next day folds too, by its weekday; yesterday and today stay as they were.
  const two = shape(20);
  expect(two).toHaveLength(16);
  expect(two[1]).toBe('day Monday · 14 messages from you');
  expect(two[2]).toBe('you You · 09:00');

  // Every earlier day folded; today's ticks are all still there.
  expect(shape(11)).toEqual([
    expect.stringMatching(/^day .* · 2 messages from you$/),
    'day Monday · 14 messages from you',
    'day Tuesday · 3 messages from you',
    'you You · 09:00',
    'lead Mira · 09:00',
    'you You · 09:01',
    'lead Mira · 09:01',
    'you You · 09:02',
    'lead Mira · 09:02',
    'you You · 09:03',
    'lead Mira · 09:03',
  ]);
});

test("when today alone is too many, its replies go and the person's messages stay", () => {
  expect(shape(10).slice(3)).toEqual([
    'you You · 09:00',
    'you You · 09:01',
    'you You · 09:02',
    'you You · 09:03',
  ]);
  // A reply dropped from the rail still belongs to the message before it.
  expect(mapEntries(chat, 10, now).map((e) => e.from)).toEqual([0, 4, 32, 38, 40, 42, 44]);
});

test('a folded day with one message, or none from the person, says so', () => {
  const lone: Tick[] = [
    { id: 'a', kind: 'you', who: 'You', time: at(9, 9), text: 'Ask' },
    { id: 'b', kind: 'lead', who: 'Mira', time: at(10, 0, 5), text: 'Late reply' },
    { id: 'c', kind: 'lead', who: 'Mira', time: at(10, 0, 9), text: 'Another' },
    ...day(11, 2),
  ];
  expect(
    mapEntries(lone, 6, now)
      .slice(0, 2)
      .map((e) => e.title),
  ).toEqual(['Monday · 1 message from you', 'Tuesday · 2 replies']);
});

test("a chat last used on an earlier day keeps that day's ticks, and what still can't fit folds as Earlier", () => {
  // Nothing today: Tuesday is the chat's latest day.
  const old = [...day(9, 3), ...day(10, 6)];
  expect(mapEntries(old, 13, now).map((e) => `${e.kind} ${e.title}`)).toEqual([
    'day Monday · 3 messages from you',
    ...Array.from({ length: 6 }, (_, n) => [`you You · 09:0${n}`, `lead Mira · 09:0${n}`]).flat(),
  ]);
  // Room for four: Tuesday's replies go, then its oldest messages and Monday fold into one tick.
  const tight = mapEntries(old, 4, now);
  expect(tight.map((e) => `${e.kind} ${e.title}`)).toEqual([
    'day Earlier · 6 messages from you',
    'you You · 09:03',
    'you You · 09:04',
    'you You · 09:05',
  ]);
  expect(tight[0]).toMatchObject({ id: 'u9-0', from: 0, preview: 'Jumps to the first of them' });
});
