// A chat that ran long with one agent and then became a team pays nothing per block for the part before (0075): its
// draw takes no longer than the same chat drawn all as a team. Timed on the real draw, the best of nine each.
import { expect, test } from 'vitest';
import type { Message, Turn } from '../api';
import { toThread } from './messages';
import { type ChatState, empty } from './state';

/** `n` turns of the person asking and the lead answering; the second agent joined halfway. */
function chat(n: number): ChatState {
  const t0 = Date.UTC(2026, 10, 1);
  const iso = (i: number) => new Date(t0 + i * 60_000).toISOString();
  const turns: Record<string, Turn> = {};
  const messages: Message[] = [];
  for (let i = 0; i < n; i++) {
    const id = `t${i}`;
    turns[id] = { id, agent: 'a', status: 'done', why: null, ref: id, position: i, topic: `m:u${i}` };
    const said = { turn: id, kind: 'text' } as const;
    messages.push({ ...said, id: `u${i}`, time: iso(2 * i), role: 'user', body: `Made-up ask ${i}` });
    messages.push({
      ...said,
      id: `a${i}`,
      time: iso(2 * i + 1),
      role: 'assistant',
      body: `Made-up answer ${i}`,
    });
  }
  return {
    ...empty,
    turns,
    messages,
    members: [
      { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
      { agent: 'b', name: 'Made-up Member', role: 'member' },
    ],
    teamSince: iso(n),
  };
}

const best = (state: ChatState) => {
  let fastest = Number.POSITIVE_INFINITY;
  for (let i = 0; i < 9; i++) {
    const t = performance.now();
    toThread(state, [], [], new Map());
    fastest = Math.min(fastest, performance.now() - t);
  }
  return fastest;
};

test('24,000 messages, half from before the team, draw no slower than the same drawn all as a team', () => {
  const half = best(chat(12_000));
  const team = best({ ...chat(12_000), teamSince: undefined });
  expect(
    half / team,
    `${half.toFixed(1)} ms half one timeline, ${team.toFixed(1)} ms all threads`,
  ).toBeLessThan(1.1);
});
