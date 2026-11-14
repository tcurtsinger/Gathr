// A task's page says what its owner reported and what it still waits on, apart from its state (0047).
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Task } from '../api';
import { TaskView } from './TaskView';

const task = (more: Partial<Task>): Task => ({
  number: 12,
  chat: 'c1',
  giver: 'mira',
  owner: 'sol',
  given_in: 't1',
  kind: 'work',
  what: 'Add the export button',
  state: 'working',
  reason: null,
  after: [],
  created_us: 0,
  ...more,
});

/** The page's fields as "label: value" lines, in order. */
async function fields(shown: Task) {
  const screen = await render(
    <TaskView task={shown} name={(a) => a ?? 'you'} onSettle={async () => {}} onReference={() => {}} />,
  );
  await expect.element(screen.getByRole('heading', { name: shown.what })).toBeVisible();
  const list = screen.container.querySelector('.task-fields');
  const terms = [...(list?.querySelectorAll('dt') ?? [])];
  return terms.map((dt) => `${dt.textContent}: ${dt.nextElementSibling?.textContent}`);
}

test('a held task shows its report and what it waits on, each once', async () => {
  const held = task({
    state: 'held',
    reason: 'done, waiting on #18 to merge',
    pr: 18,
    report: { how: 'done', note: 'ready for review', see: 'src/export.ts', at_us: 1 },
    waiting_on: ['#18 to merge'],
  });
  expect(await fields(held)).toEqual([
    'Assigned to: sol',
    'State: held',
    'Report: Done: ready for review · src/export.ts',
    'Waiting on: #18 to merge',
  ]);
});

test('a done task shows its note once, and a merge as its reason', async () => {
  const report = { how: 'done' as const, note: 'Updated the guide', see: null, at_us: 1 };
  const noted = task({ state: 'done', reason: 'Updated the guide', report });
  expect(await fields(noted)).toEqual(['Assigned to: sol', 'State: done', 'Report: Done: Updated the guide']);
});

test('a task done by its merge shows the merge beside the report', async () => {
  const report = { how: 'done' as const, note: 'Updated the guide', see: null, at_us: 1 };
  const merged = task({ state: 'done', reason: '#18 merged', report });
  expect(await fields(merged)).toEqual([
    'Assigned to: sol',
    'State: done',
    'Reason: #18 merged',
    'Report: Done: Updated the guide',
  ]);
});

test('a task stopped as left says what is left', async () => {
  const left = task({
    state: 'blocked',
    reason: 'left: the dark theme and the PR remain',
    report: { how: 'left', note: 'the dark theme and the PR remain', see: null, at_us: 1 },
  });
  expect(await fields(left)).toEqual([
    'Assigned to: sol',
    'State: blocked',
    'Report: Left: the dark theme and the PR remain',
  ]);
});

test('a task under way with no report says so', async () => {
  expect(await fields(task({}))).toEqual(['Assigned to: sol', 'State: working', 'Report: No report yet']);
});

test('a task asked for its report says it was asked', async () => {
  expect(await fields(task({ state: 'ready', asked: true }))).toEqual([
    'Assigned to: sol',
    'State: ready',
    'Report: No report yet. Asked once',
  ]);
});

test('a task blocked for the one it comes after keeps its reason', async () => {
  const waiting = task({ state: 'blocked', reason: '#3 is blocked' });
  expect(await fields(waiting)).toEqual(['Assigned to: sol', 'State: blocked', 'Reason: #3 is blocked']);
});

test('a task reported blocked on another, with no note, still names that task', async () => {
  const on = task({
    state: 'blocked',
    reason: 'blocked on #3',
    report: { how: 'blocked', note: null, see: null, at_us: 1 },
  });
  expect(await fields(on)).toEqual([
    'Assigned to: sol',
    'State: blocked',
    'Reason: blocked on #3',
    'Report: Blocked',
  ]);
});

test("a review shows the commit it is of, and the PR's head once it is out of date", async () => {
  const review = task({
    kind: 'review',
    state: 'done',
    pr: 18,
    reviewed: 'abc1234def',
    pr_head: 'abc1234def5678',
    report: { how: 'done', note: null, see: null, at_us: 1 },
  });
  expect(await fields(review)).toEqual([
    'Assigned to: sol',
    'State: done',
    'Report: Done',
    'Reviewed at: abc1234',
  ]);
  const stale = { ...review, number: 13, what: 'Review the export', pr_head: 'def5678aaaa', outdated: true };
  expect(await fields(stale)).toEqual([
    'Assigned to: sol',
    'State: done',
    'Report: Done',
    'Reviewed at: abc1234. Out of date: #18 is at def5678',
  ]);
});

test('a review lists its notes, each with its kind and file', async () => {
  const review = task({
    kind: 'review',
    state: 'blocked',
    reason: 'left: two notes. Gathr gave #14 to sol to fix it',
    pr: 18,
    reviewed: 'abc1234def',
    report: { how: 'left', note: 'two notes', see: null, at_us: 1 },
    notes: [
      { kind: 'wrong', file: 'src/export.ts', says: 'The export skips archived chats' },
      { kind: 'unasked', file: 'src/cache.rs', says: 'A cache the ask does not need' },
      { kind: 'suggestion', file: null, says: 'Name the button Export' },
    ],
  });
  expect(await fields(review)).toContain('Notes: Wrong · src/export.ts: The export skips archived chats');
  const notes = [...document.querySelectorAll('.task-fields dt')].find((dt) => dt.textContent === 'Notes');
  const lines: string[] = [];
  for (let dd = notes?.nextElementSibling; dd?.tagName === 'DD'; dd = dd.nextElementSibling)
    lines.push(dd.textContent ?? '');
  expect(lines).toEqual([
    'Wrong · src/export.ts: The export skips archived chats',
    'Unasked · src/cache.rs: A cache the ask does not need',
    'Suggestion · Name the button Export',
  ]);
});
