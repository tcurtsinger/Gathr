// A plan check's findings as a chip at the end of its checker's bubble (0078), drawn from the service's recorded answers
// with the check left instead of agreed: what it says, and its notes opened and closed by pointer and keyboard. Only a
// plan check has it: a code review's notes stay in the Tasks view.
import { expect, test } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { ReviewNote } from '../api';
import { chat, View } from './recorded';
import type { ChatState } from './state';

/**
 * The recorded ask with its plan's check left instead of agreed: Rev's check task blocked with `notes`, the version
 * never sent, nothing approved or handed out, and Gathr's line on the check as the service words it.
 */
function leftWith(notes: ReviewNote[]): ChatState {
  const state = chat('askAtWork');
  const plan = state.plans[0];
  const version = plan.versions[0];
  const line = version.check_line as string;
  const at = state.messages.findIndex((m) => m.id === line);
  const check = state.tasks.find((t) => t.checks === version.id);
  if (!check || at < 0) throw new Error('the recorded chat has no plan check');
  const found = notes.filter((n) => n.kind !== 'suggestion').length;
  const said = `Rev has ${found} findings on version 1 of the plan "${plan.title}".`;
  return {
    ...state,
    messages: [...state.messages.slice(0, at), { ...state.messages[at], body: said }],
    turns: Object.fromEntries(
      Object.entries(state.turns).filter(([, t]) => t.task == null || t.task === check.number),
    ),
    tasks: [
      {
        ...check,
        state: 'blocked',
        report: { how: 'left', note: null, see: null, at_us: check.report?.at_us ?? null },
        notes,
      },
    ],
    plans: [
      {
        ...plan,
        state: 'revising',
        versions: [{ ...version, sent: null, decided: null, decided_line: null }],
      },
    ],
    reactions: [],
  };
}

const SEVEN: ReviewNote[] = [
  { kind: 'wrong', file: 'src/tags.rs', says: 'Made-up: the tags table has no index on note' },
  { kind: 'wrong', file: null, says: 'Made-up: part 2 runs before part 1 lands' },
  { kind: 'unasked', file: 'src/export.rs', says: 'Made-up: the export is not part of the ask' },
  { kind: 'wrong', file: 'tests/tags.rs', says: 'Made-up: the proof checks nothing' },
  { kind: 'wrong', file: null, says: 'Made-up: Ann owns both parts' },
  { kind: 'unasked', file: null, says: 'Made-up: a tag colour picker' },
  { kind: 'wrong', file: 'src/notes.rs', says: 'Made-up: a missed caller' },
];

/** The drawn block whose name line names `who`, main message or reply. */
const said = (who: string) =>
  [...document.querySelectorAll<HTMLElement>('.thread-messages > .said')].find(
    (el) => el.querySelector('.name-line-name')?.textContent === who,
  );

/** Opens the lead's thread, where the check is, if it starts folded. */
async function openThread() {
  await expect.poll(() => document.querySelector('.plan-chip')).toBeTruthy();
  const fold = document.querySelector<HTMLButtonElement>('.said-foot .fold-line');
  if (fold?.getAttribute('aria-expanded') === 'false') await fold.click();
  await expect.poll(() => said('Rev')).toBeDefined();
}

test("a check that left findings ends its checker's bubble with 7 findings, opening to each note by pointer", async () => {
  await render(<View state={leftWith(SEVEN)} />);
  await openThread();
  const chip = page.getByRole('button', { name: '7 findings' });
  await expect.element(chip).toBeVisible();
  const bubble = said('Rev')?.querySelector('.bubble') as HTMLElement;
  expect(bubble.contains(chip.element())).toBe(true);
  // Its last piece: after the checker's words.
  const words = [...bubble.querySelectorAll('p')].find((p) => p.textContent === 'Hi!') as HTMLElement;
  expect(words.compareDocumentPosition(chip.element()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(chip.element().getAttribute('aria-expanded')).toBe('false');
  expect(bubble.textContent).not.toContain('Made-up: the tags table');
  await chip.click();
  await expect.poll(() => chip.element().getAttribute('aria-expanded')).toBe('true');
  const notes = [...bubble.querySelectorAll('.note')];
  expect(notes.map((n) => n.textContent)).toEqual([
    'Wrong · src/tags.rs: Made-up: the tags table has no index on note',
    'Wrong · Made-up: part 2 runs before part 1 lands',
    'Not asked for · src/export.rs: Made-up: the export is not part of the ask',
    'Wrong · tests/tags.rs: Made-up: the proof checks nothing',
    'Wrong · Made-up: Ann owns both parts',
    'Not asked for · Made-up: a tag colour picker',
    'Wrong · src/notes.rs: Made-up: a missed caller',
  ]);
  // The file in monospace, when the note names one.
  const file = notes[0].querySelector('code') as HTMLElement;
  expect(file.textContent).toBe('src/tags.rs');
  expect(getComputedStyle(file).fontFamily).toMatch(/mono|consolas/i);
  expect(notes[1].querySelector('code')).toBeNull();
  await chip.click();
  await expect.poll(() => bubble.querySelectorAll('.note').length).toBe(0);
  expect(chip.element().getAttribute('aria-expanded')).toBe('false');
});

test('the findings chip opens and closes by keyboard', async () => {
  await render(<View state={leftWith(SEVEN.slice(0, 1))} />);
  await openThread();
  const chip = page.getByRole('button', { name: '1 finding' });
  await expect.element(chip).toBeVisible();
  (chip.element() as HTMLButtonElement).focus();
  await userEvent.keyboard('{Enter}');
  await expect
    .poll(() => [...document.querySelectorAll('.note')].map((n) => n.textContent))
    .toEqual(['Wrong · src/tags.rs: Made-up: the tags table has no index on note']);
  expect(chip.element().getAttribute('aria-expanded')).toBe('true');
  await userEvent.keyboard('{Enter}');
  await expect.poll(() => document.querySelectorAll('.note').length).toBe(0);
});

test("a check that left findings has no findings line of Gathr's, Plan v1 and its count", async () => {
  await render(<View state={leftWith(SEVEN)} />);
  await openThread();
  await expect.element(page.getByRole('button', { name: '7 findings' })).toBeVisible();
  const text = document.querySelector('.thread-messages')?.textContent ?? '';
  expect(text).not.toContain('Plan v1:');
  expect(document.querySelectorAll('.event-line')).toHaveLength(0);
});

test('a report of suggestions only reads "2 suggestions"', async () => {
  const ideas: ReviewNote[] = [
    { kind: 'suggestion', file: null, says: 'Made-up: name the column tag' },
    { kind: 'suggestion', file: 'src/tags.rs', says: 'Made-up: one query for both' },
  ];
  await render(<View state={leftWith(ideas)} />);
  await openThread();
  const chip = page.getByRole('button', { name: '2 suggestions' });
  await expect.element(chip).toBeVisible();
  await chip.click();
  await expect
    .poll(() => [...document.querySelectorAll('.note')].map((n) => n.textContent))
    .toEqual([
      'Suggestion · Made-up: name the column tag',
      'Suggestion · src/tags.rs: Made-up: one query for both',
    ]);
});

test("a check that agrees has no chip: only the checker's thumbs-up on the lead's bubble", async () => {
  await render(<View state={chat('askAtWork')} />);
  await openThread();
  await expect.poll(() => said('Lead')?.querySelector('.said-foot .react')).toBeTruthy();
  expect(document.querySelector('.findings-chip')).toBeNull();
  expect(said('Rev')?.querySelector('.bubble')?.textContent).toBe('Hi!');
});

test("a code review's notes show no chip in the reviewer's bubble", async () => {
  const state = chat('reviewAndFix');
  expect(state.tasks.some((t) => t.kind === 'review' && t.notes?.length)).toBe(true);
  await render(<View state={state} />);
  await expect.poll(() => said('Vale')).toBeDefined();
  expect(document.querySelector('.findings-chip')).toBeNull();
  expect(document.body.textContent).not.toContain("feature.txt isn't part of the ask");
});
