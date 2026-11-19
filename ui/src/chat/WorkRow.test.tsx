// A turn's work row as assistant-ui's Run activity and Tool call (0044, the owner's call of 2026-11-12).
import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Work } from './fold';
import { WorkRow } from './TurnParts';

const finished: Work = {
  running: false,
  label: 'Worked for 1m 49s',
  notes: ['3 steps', 'gave 1 task'],
  steps: [
    {
      id: 't1',
      label: 'Ran',
      doing: 'Running',
      detail: 'cargo nextest run',
      state: 'ok',
      request: 'cargo nextest run \\n  --workspace',
      result: 'Summary 360 tests run: 360 passed\n…',
    },
    { id: 's', say: 'Two tasks are stale.' },
    {
      id: 't2',
      label: 'Ran',
      doing: 'Running',
      detail: 'npx vitest run',
      state: 'error',
      request: 'npx vitest run',
    },
    {
      id: 't3',
      label: 'Edited',
      doing: 'Editing',
      detail: 'notes.txt',
      state: 'stopped',
      request: 'notes.txt',
    },
    {
      id: 't4',
      label: 'Gave Kit a task',
      doing: 'Giving a task',
      detail: '#7 Open the PR',
      task: 7,
      state: 'ok',
    },
  ],
};

test('a finished row opens to its steps, and a step to what it ran and what came back', async () => {
  const opened: number[] = [];
  const screen = await render(<WorkRow work={finished} onOpenTask={(n) => opened.push(n)} />);
  const row = screen.getByRole('button', { name: /Worked for 1m 49s · 3 steps · gave 1 task/ });
  await row.click();
  await expect.element(screen.getByText('Two tasks are stale.')).toBeVisible();
  await expect.element(screen.getByRole('img', { name: 'Failed' })).toBeVisible();
  await expect.element(screen.getByText('Stopped', { exact: true })).toBeVisible();

  await screen.getByRole('button', { name: /^Ran cargo nextest run/ }).click();
  const request = screen.getByText(/--workspace/);
  await expect.element(request).toBeVisible();
  // A command's lines stay lines, and "…" says more came back.
  expect((request.element() as HTMLElement).innerText).toBe('cargo nextest run \\n  --workspace');
  const result = screen.getByText(/360 passed/);
  expect((result.element() as HTMLElement).innerText).toBe('Summary 360 tests run: 360 passed\n…');

  await screen.getByRole('button', { name: '#7 Open the PR' }).click();
  expect(opened).toEqual([7]);
});

test('a running row says Working, with its latest step under it', async () => {
  const running: Work = {
    running: true,
    label: 'Running',
    notes: ['1 step'],
    steps: [
      {
        id: 't1',
        label: 'Ran',
        doing: 'Running',
        detail: 'cargo build',
        state: 'running',
        request: 'cargo build',
      },
    ],
  };
  await render(<WorkRow work={running} onOpenTask={() => {}} />);
  await expect.element(page.getByRole('button', { name: /^Working/ })).toBeVisible();
  await expect.element(page.getByText('Running cargo build')).toBeVisible();
});

// Seen in use, 2026-11-19: the row's shimmer showed a second, smaller "Working" at its corner, the copy assistant-ui
// keeps for screen readers, which a text mask shows whatever clips it.
test('a working row’s copy for screen readers has no glyph for its shimmer to show', async () => {
  const working: Work = { running: true, label: 'Working', notes: [], steps: finished.steps.slice(0, 1) };
  const screen = await render(<WorkRow work={working} onOpenTask={() => {}} />);
  const status = screen.getByRole('status');
  await expect.element(status).toHaveTextContent('Working');
  expect(getComputedStyle(status.element()).fontSize).toBe('0px');
});

// Seen in use, 2026-11-19: while an agent worked, the chat had a sideways scroll bar. The shimmer's highlight is a
// box that travels past the row's right edge, and what it reaches counted as the chat's width.
test('a working row’s shimmer adds no width to what holds it', async () => {
  const working: Work = { running: true, label: 'Working', notes: [], steps: finished.steps.slice(0, 1) };
  // The highlight held at the end of its travel, where it reaches furthest.
  const held = document.createElement('style');
  held.textContent = '.work.shimmer::before{animation:none!important;translate:var(--_travel) 0!important}';
  document.head.append(held);
  await render(
    <div data-testid="holds" style={{ width: 420, overflow: 'auto' }}>
      <WorkRow work={working} onOpenTask={() => {}} />
    </div>,
  );
  const holds = page.getByTestId('holds').element();
  await expect.element(page.getByRole('button', { name: /^Working/ })).toBeVisible();
  expect(holds.scrollWidth).toBe(holds.clientWidth);
  held.remove();
});
