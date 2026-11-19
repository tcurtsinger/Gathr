import { expect, test } from 'vitest';
import type { History, Message } from '../api';
import type { LiveEvent } from '../live';
import { apply, empty, fromLoad } from './state';

const update = (status: 'running' | 'failed', agent: string | null) =>
  ({ type: 'turn', chat: 'c', turn: 't', status, agent }) as const;

test('a turn sent from another window knows its agent from its live updates alone', () => {
  const failed = apply(apply(empty, update('running', 'a')), update('failed', 'a'));
  expect(failed.turns.t).toMatchObject({ status: 'failed', agent: 'a' });

  const noAgent = apply(empty, update('failed', null));
  expect(noAgent.turns.t).toMatchObject({ status: 'failed', agent: null });
});

test("an agent's post moves its running turn by the topic alone, keeping what the turn already held", () => {
  const queued: LiveEvent = { type: 'turn', chat: 'c', turn: 't', status: 'running', agent: 'a', ref: 'r1' };
  const held = apply(empty, {
    ...queued,
    task: 3,
    answers: 'u',
    topic: 't:3',
    created: '2026-11-19T09:00:00Z',
  });
  const moved = apply(held, {
    type: 'turn',
    chat: 'c',
    turn: 't',
    status: 'running',
    agent: 'a',
    topic: 'b:t',
  });
  expect(moved.turns.t).toMatchObject({
    topic: 'b:t',
    task: 3,
    answers: 'u',
    ref: 'r1',
    created: '2026-11-19T09:00:00Z',
  });
});

const delta = (text: string, at: number): LiveEvent => ({ type: 'delta', chat: 'c', turn: 't', text, at });
const block = (id: string, body: string): Message => ({
  id,
  turn: 't',
  time: '2026-11-08T09:00:00Z',
  role: 'assistant',
  kind: 'text',
  body,
});
const history = (messages: Message[], streaming: string): History =>
  ({
    messages,
    turns: [{ id: 't', agent: 'a', status: 'running', why: null, ref: 't' }],
    approvals: [],
    members: [],
    streaming: streaming ? [{ turn: 't', agent: 'a', text: streaming }] : [],
  }) as unknown as History;
const work = { tasks: [], waits: [] };

test('a chat opened mid-stream shows each streamed piece once, whatever the history already held', () => {
  // The socket listens before the history is read, so what came in meanwhile may be in it already.
  const held = [delta('Partial ', 0), delta('once.', 8), delta(' More', 13)];
  expect(fromLoad(history([], 'Partial once.'), work, held).partial.t).toBe('Partial once. More');
  // A piece the history holds part of adds only the rest.
  expect(fromLoad(history([], 'Partial on'), work, held).partial.t).toBe('Partial once. More');
  expect(fromLoad(history([], ''), work, held).partial.t).toBe('Partial once. More');

  // A block saved before the read: its pieces are in that message, and the next block starts over.
  const saved = block('m1', 'First block.');
  const across: LiveEvent[] = [
    delta('First ', 0),
    delta('block.', 6),
    { type: 'message', chat: 'c', message: saved },
    delta('Second', 0),
    delta(' block', 6),
  ];
  const state = fromLoad(history([saved], 'Second'), work, across);
  expect(state.messages.map((m) => m.body)).toEqual(['First block.']);
  expect(state.partial.t).toBe('Second block');

  // A saved block the history holds never clears the newer block it is still streaming.
  const followUp: Message = { ...block('m2', 'Follow-up'), role: 'user', turn: 'u' };
  const overlap: LiveEvent[] = [
    delta('First block.', 0),
    { type: 'message', chat: 'c', message: saved },
    delta('Second partial.', 0),
    { type: 'message', chat: 'c', message: followUp },
  ];
  const both = fromLoad(history([saved, followUp], 'Second partial.'), work, overlap);
  expect(both.messages.map((m) => m.body)).toEqual(['First block.', 'Follow-up']);
  expect(both.partial.t).toBe('Second partial.');
});

test('a thumbs-up both in the history and held from the socket shows once', () => {
  const kept = { message: 'm1', agent: 'b', time: '2026-11-08T09:01:00Z' };
  const later = { message: 'm1', agent: 'c', time: '2026-11-08T09:02:00Z' };
  const held: LiveEvent[] = [
    { type: 'reaction', chat: 'c', reaction: kept },
    { type: 'reaction', chat: 'c', reaction: later },
  ];
  const loaded = { ...history([block('m1', 'Made-up words')], ''), reactions: [kept] };
  expect(fromLoad(loaded, work, held).reactions).toEqual([kept, later]);
});

// Seen in use, 2026-11-19: a check turn followed live had no `opened`, so Gathr's words to its agent were drawn as a
// line and then as the bubble's quote, and its Checked line was missing until a reload.
test('a live turn keeps what opened it, as a reload reads it (0080)', () => {
  const queued: LiveEvent = {
    type: 'turn',
    chat: 'c',
    turn: 'k1',
    status: 'queued',
    agent: 'b',
    answers: 't1',
    opened: 'check',
  };
  const running: LiveEvent = { type: 'turn', chat: 'c', turn: 'k1', status: 'running', agent: 'b' };
  expect(apply(apply(empty, queued), running).turns.k1.opened).toBe('check');
});

// The last change of this kind failed live: the reducer dropped a field the live event carried (0082's brief).
test('a live turn keeps when it was made and when it ended, through every later event of it', () => {
  const made = '2026-11-19T10:00:00Z';
  const turn = (status: 'queued' | 'running' | 'done', more = {}): LiveEvent => ({
    type: 'turn',
    chat: 'c',
    turn: 'k1',
    status,
    agent: 'b',
    ...more,
  });
  const steps = [
    turn('queued', { created: made }),
    turn('running'),
    turn('done', { ended: '2026-11-19T10:00:14Z' }),
  ];
  const state = steps.reduce(apply, empty);
  expect([state.turns.k1.created, state.turns.k1.ended]).toEqual([made, '2026-11-19T10:00:14Z']);
});

test("a live context update moves that member's fill and window alone", () => {
  const state = {
    ...empty,
    members: [
      { agent: 'a', name: 'Made-up A', role: 'supervisor' as const, context: null },
      { agent: 'b', name: 'Made-up B', role: 'member' as const, context: { used: 10, window: 100 } },
    ],
  };
  const event: LiveEvent = {
    type: 'context',
    chat: 'c',
    agent: 'a',
    context: { used: 3_611, window: 1_000_000 },
  };
  const next = apply(state, event);
  expect(next.members.map((m) => m.context)).toEqual([
    { used: 3_611, window: 1_000_000 },
    { used: 10, window: 100 },
  ]);
});
