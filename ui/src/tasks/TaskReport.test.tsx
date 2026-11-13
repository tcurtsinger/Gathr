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
