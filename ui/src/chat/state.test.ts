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
