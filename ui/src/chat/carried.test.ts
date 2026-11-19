// Gathr's copy of the person's message to several agents (0028) entering an agent's running turn: the copy draws
// nothing, but the turn splits there, and what the agent says after it sits in the message's thread. Made-up words.
import { expect, test } from 'vitest';
import type { Message, Turn } from '../api';
import { type Group, grouped } from './groups';
import { type ChatState, empty } from './state';
import { topicsOf } from './topics';

const turn = (id: string, agent: string, position: number, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  position,
  created: `2026-11-08T10:0${position}:00Z`,
  ...more,
});
const said = (
  id: string,
  turnId: string,
  minute: number,
  body: string,
  more: Partial<Message> = {},
): Message => ({
  id,
  turn: turnId,
  time: `2026-11-08T10:${String(minute).padStart(2, '0')}:00Z`,
  role: 'assistant',
  kind: 'text',
  body,
  ...more,
});

test('a copy of the person’s message splits the running turn it entered: what follows sits in its thread', () => {
  // As history orders it: Bo's running turn (placed first) with its words before and after the copy, then the lead's.
  const state: ChatState = {
    ...empty,
    members: [
      { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
      { agent: 'b', name: 'Bo', role: 'member' },
    ],
    turns: {
      r: turn('r', 'b', 1, { status: 'running', topic: 'm:q0' }),
      t1: turn('t1', 'a', 2, { topic: 'm:p' }),
    },
    messages: [
      said('q0', 'r', 1, 'Made-up earlier ask', { role: 'user' }),
      said('w1', 'r', 2, 'Made-up words before'),
      said('k', 'r', 4, 'Made-up line', { role: 'gathr', copy_of: 'p' }),
      said('w2', 'r', 5, 'Made-up words after'),
      said('p', 't1', 3, '@Made-up Lead @Bo both of you', { role: 'user' }),
      said('l1', 't1', 4, 'Made-up lead words'),
    ],
  };
  const groups = grouped(state);
  const topics = topicsOf(groups, state, new Map(), new Map());
  const of = (id: string) => topics.get(groups.find((g) => g.msgs.some((m) => m.id === id)) as Group);
  expect([of('w1'), of('w2'), of('l1')]).toEqual(['m:q0', 'm:p', 'm:p']);
  // The copy itself is no block.
  expect(groups.some((g) => g.msgs.some((m) => m.id === 'k'))).toBe(false);
});
