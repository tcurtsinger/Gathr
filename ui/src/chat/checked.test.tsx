// The open check in the window (0080): a member's check that wrote words opens its bubble with "Checked Mira's
// answer" under the name line, its quote and words as Markdown after it; a check that only gave a thumbs-up draws no
// bubble, only that thumbs-up on the answer; Gathr's words that opened either, or the lead's last word, draw nowhere.
// Made-up names and words only.
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Message, Turn } from '../api';
import { View } from './recorded';
import { type ChatState, empty } from './state';

const members = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' as const },
  { agent: 'ann', name: 'Ann', role: 'member' as const },
  { agent: 'bo', name: 'Bo', role: 'reviewer' as const },
];

const turn = (id: string, agent: string, position: number, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  position,
  created: `2026-11-19T10:0${position}:00Z`,
  topic: 'b:t1',
  ...more,
});

const said = (
  id: string,
  turnId: string,
  minute: number,
  body: unknown,
  more: Partial<Message> = {},
): Message => ({
  id,
  turn: turnId,
  time: `2026-11-19T10:${String(minute).padStart(2, '0')}:00Z`,
  role: 'assistant',
  kind: 'text',
  body: body as string,
  ...more,
});

const CHECK = "Made-up: check Mira's answer before the person relies on it.";

/** The person asks; Mira answers; Ann checks it in words; Bo only gives a thumbs-up; Mira has the last word. */
function state(): ChatState {
  const check = { opened: 'check', answers: 't1' };
  return {
    ...empty,
    members,
    turns: {
      t1: turn('t1', 'mira', 1),
      ann: turn('ann', 'ann', 2, check),
      bo: turn('bo', 'bo', 3, check),
      last: turn('last', 'mira', 4, { opened: 'last_word', answers: 't1' }),
    },
    messages: [
      said('p1', 't1', 1, 'Made-up: show tags in the list', { role: 'user' }),
      said('a1', 't1', 2, 'Made-up: tags show in the list now.'),
      said('ann-check', 'ann', 3, CHECK, { role: 'gathr' }),
      said(
        'ann-said',
        'ann',
        4,
        '> Mira: tags show in the list now.\n\nMade-up: the filter still ignores them.',
      ),
      said('bo-check', 'bo', 5, CHECK, { role: 'gathr' }),
      said('bo-react', 'bo', 6, { tool: 'mcp__gathr__react', status: 'ok' }, { kind: 'tool' }),
      said('last-told', 'last', 7, 'Made-up: Ann questioned your answer.', { role: 'gathr' }),
      said('last-said', 'last', 8, 'Made-up: Ann is right, the filter is next.'),
    ],
    reactions: [{ message: 'a1', agent: 'bo', time: '2026-11-19T10:06:00Z' }],
  };
}

/** Opens Mira's thread, where the checks are, if it starts folded. */
async function openThread() {
  await expect.poll(() => document.querySelector('.said-foot .fold-line')).toBeTruthy();
  const fold = document.querySelector<HTMLButtonElement>('.said-foot .fold-line');
  if (fold?.getAttribute('aria-expanded') === 'false') await fold.click();
  await expect.poll(() => block('Ann')).toBeDefined();
}

/** The drawn block whose name line names `who`. */
const block = (who: string) =>
  [...document.querySelectorAll<HTMLElement>('.said')].find(
    (el) => el.querySelector('.name-line-name')?.textContent === who,
  );

test('a check in words opens its bubble with the line saying whose answer it checked, then its quote and words', async () => {
  await render(<View state={state()} />);
  await openThread();
  const bubble = block('Ann')?.querySelector('.bubble') as HTMLElement;
  const line = bubble.querySelector('.checked-line') as HTMLElement;
  expect(line.textContent).toBe("Checked Mira's answer");
  expect(line.querySelector('svg')).not.toBeNull();
  expect(bubble.firstElementChild).toBe(line);
  const quote = bubble.querySelector('blockquote') as HTMLElement;
  expect(quote.textContent?.trim()).toBe('Mira: tags show in the list now.');
  expect(line.compareDocumentPosition(quote) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(bubble.textContent).toContain('Made-up: the filter still ignores them.');
});

test('a check that only gave a thumbs-up draws no bubble, only the thumbs-up on the answer', async () => {
  await render(<View state={state()} />);
  await openThread();
  expect(document.querySelector('.said[data-agent="bo"]')).toBeNull();
  const answer = block('Mira') as HTMLElement;
  expect(answer.querySelector('.react')?.getAttribute('aria-label')).toBe('Thumbs-up from Bo');
});

test("Gathr's words that opened a check or the last word draw nowhere", async () => {
  await render(<View state={state()} />);
  await openThread();
  await expect.poll(() => document.body.textContent).toContain('Made-up: Ann is right, the filter is next.');
  const text = document.body.textContent ?? '';
  expect(text).not.toContain(CHECK);
  expect(text).not.toContain('Ann questioned your answer');
  expect(document.querySelectorAll('.checked-line')).toHaveLength(1);
});
