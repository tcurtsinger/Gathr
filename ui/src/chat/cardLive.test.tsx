// A member's check followed LIVE (0082): the window's state built from the live events alone, as the socket sends them
// (a turn queued, Gathr's words to it, running, words streaming, a step, more words, done), drawn after each one. The
// bubble shows from the moment the turn runs and never blanks; words being written show as words, and fold into the
// working line once a step follows them; at the end the message is the words after the last step, the line at the
// foot what it did, and the name line how long it took. Made-up names and words only.
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Message, ToolBody } from '../api';
import type { LiveEvent } from '../live';
import { View } from './recorded';
import { apply, type ChatState, empty } from './state';

const members = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' as const },
  { agent: 'ann', name: 'Ann', role: 'member' as const },
];

const at = (s: number) => `2026-11-19T10:00:${String(s).padStart(2, '0')}Z`;
const message = (
  id: string,
  turn: string,
  s: number,
  body: unknown,
  more: Partial<Message> = {},
): LiveEvent => ({
  type: 'message',
  chat: 'c',
  message: { id, turn, time: at(s), role: 'assistant', kind: 'text', body: body as string, ...more },
});
const turn = (id: string, agent: string, status: 'queued' | 'running' | 'done', more = {}): LiveEvent => ({
  type: 'turn',
  chat: 'c',
  turn: id,
  status,
  agent,
  ...more,
});
const bash = (status: ToolBody['status']): ToolBody => ({
  tool: 'Bash',
  status,
  input: { text: 'cargo test' },
});

/** The person asks, Mira answers: the chat as it stood before Ann's check, loaded from history. */
const before: ChatState = [
  { type: 'members', chat: 'c', members } as LiveEvent,
  turn('t1', 'mira', 'running', { created: at(0), position: 1, topic: 'b:t1' }),
  message('p1', 't1', 0, 'Made-up: why is the list slow?', { role: 'user' }),
  message('a1', 't1', 1, 'Made-up: it reads every note’s tags one by one.'),
  turn('t1', 'mira', 'done', { ended: at(2) }),
].reduce(apply, empty);

const check = { answers: 't1', opened: 'check', topic: 'b:t1' };
const NARRATION = 'Made-up: I’ll run the tests first.';
const OBJECTION = '> Mira: it reads every note’s tags one by one.\n\nMade-up: the filter is slow too.';

/** Each live event of Ann's check, with what her bubble reads once it is drawn (null: no bubble yet). */
const steps: [LiveEvent, (string | null)[] | null][] = [
  [turn('ann', 'ann', 'queued', { ...check, created: at(3) }), null],
  [message('ann-told', 'ann', 3, 'Made-up: check Mira’s answer.', { role: 'gathr' }), null],
  [turn('ann', 'ann', 'running', { position: 2 }), ["Checked Mira's answer", '> Working']],
  [
    { type: 'delta', chat: 'c', turn: 'ann', text: 'Made-up: I’ll run' },
    ["Checked Mira's answer", 'Made-up: I’ll run', '> Working'],
  ],
  [
    { type: 'delta', chat: 'c', turn: 'ann', text: ' the tests first.' },
    ["Checked Mira's answer", NARRATION, '> Working'],
  ],
  [message('ann-1', 'ann', 5, NARRATION), ["Checked Mira's answer", NARRATION, '> Working']],
  // A step follows the words: they stay, read as words, and the working line says the step (the owner's call of
  // 2026-11-19: narration inside the shimmering line had to be deciphered).
  [
    message('ann-2', 'ann', 6, bash('running'), { kind: 'tool' }),
    ["Checked Mira's answer", NARRATION, '> WorkingRunning cargo test'],
  ],
  [
    { type: 'tool', chat: 'c', message: 'ann-2', body: bash('ok') } as LiveEvent,
    ["Checked Mira's answer", NARRATION, '> WorkingRan cargo test'],
  ],
  [
    { type: 'delta', chat: 'c', turn: 'ann', text: OBJECTION },
    [
      "Checked Mira's answer",
      'Mira: it reads every note’s tags one by one.Made-up: the filter is slow too.',
      '> WorkingRan cargo test',
    ],
  ],
  [
    message('ann-3', 'ann', 16, OBJECTION),
    [
      "Checked Mira's answer",
      'Mira: it reads every note’s tags one by one.Made-up: the filter is slow too.',
      '> WorkingRan cargo test',
    ],
  ],
  [
    turn('ann', 'ann', 'done', { ended: at(17) }),
    [
      "Checked Mira's answer",
      'Mira: it reads every note’s tags one by one.Made-up: the filter is slow too.',
      '> Ran 1 command',
    ],
  ],
];

/** Ann's bubble, piece by piece: the steps line as "> " and its label, with the line under it while it runs. */
function read(): (string | null)[] | null {
  const bubble = document.querySelector('.said[data-agent="ann"] .bubble');
  if (!bubble) return null;
  return [...bubble.children].map((piece) => {
    if (!piece.matches('.work')) return (piece.textContent ?? '').replace(/\s*\n\s*/g, '').trim();
    const label = piece.querySelector('[data-slot="collapsible-trigger"], p')?.textContent ?? '';
    const latest = piece.querySelector('[data-slot="collapsible"] > p')?.textContent ?? '';
    return `> ${label.replace(/\s*·.*$/, '').trim()}${latest}`;
  });
}

test('a check followed live: never blank once running, the latest words read as words, the message stays', async () => {
  let state = before;
  const screen = await render(<View state={state} />);
  await expect.poll(() => document.body.textContent).toContain('one by one');
  for (const [event, reads] of steps) {
    state = apply(state, event);
    await screen.rerender(<View state={state} />);
    // Mira's thread, where the check sits, if it is folded.
    document.querySelector<HTMLButtonElement>('.said-foot .fold-line[aria-expanded="false"]')?.click();
    const label = event.type === 'message' ? event.message.id : `${event.type}`;
    await expect.poll(() => [label, read()]).toEqual([label, reads]);
  }
  const ann = document.querySelector('.said[data-agent="ann"]') as Element;
  expect(ann.querySelector('.name-line-took')?.textContent).toBe('14s');
  expect(ann.querySelector('.name-line-title')).toBeNull();
});
