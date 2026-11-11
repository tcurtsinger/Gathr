// The service is the edge here: fetch answers as it would, and live events arrive when the test says.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { CheckRun } from '../api';
import type { LiveEvent } from '../live';
import { TaskChecks } from './TaskChecks';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('../live', () => ({
  listen: (on: (event: LiveEvent) => void) => {
    live.add(on);
    return () => live.delete(on);
  },
}));
afterEach(() => {
  vi.unstubAllGlobals();
  live.clear();
});

const head = '8bd41c2e5f00a1b2c3d4e5f60718293a4b5c6d7e';
const run = (more: Partial<CheckRun>): CheckRun => ({
  run: 'r1',
  head,
  state: 'passed',
  why: null,
  ref: null,
  started: '2026-11-11T10:40:00.000Z',
  ended: '2026-11-11T10:42:15.000Z',
  steps: [
    { command: 'cargo test', exit: 0, duration_ms: 108_000, log: 'test result: ok. 36 passed' },
    { command: 'npm test', exit: 0, duration_ms: 27_000, log: '18 passed' },
  ],
  ...more,
});
let runs: CheckRun[] = [];
let refuse = false;
/** The service answers one commit's runs, as the task asks for them (Vale, #121: never the newest 50). */
const served = () =>
  vi.stubGlobal('fetch', async (path: string) => {
    if (path !== `/api/projects/p/checks/runs?head=${head}`) return new Response('{}');
    if (refuse) {
      const error = { error: 'Checks failed', why: 'made-up refusal', ref: 'made-up-ref' };
      return new Response(JSON.stringify(error), { status: 500 });
    }
    return new Response(JSON.stringify({ runs: runs.filter((r) => r.head === head) }));
  });

test("a failed run says so with its reference, each command's exit and time, the failing one's log open", async () => {
  runs = [
    run({
      state: 'failed',
      why: 'npm test exited with 1',
      ref: 'made-up-ref',
      steps: [
        { command: 'cargo test', exit: 0, duration_ms: 108_000, log: 'test result: ok. 36 passed' },
        { command: 'npm test', exit: 1, duration_ms: 27_000, log: 'Expected cursor 42; received 41' },
      ],
    }),
    // An older head's run: not this commit's.
    run({ run: 'r0', head: '0000000aaaa', state: 'passed' }),
  ];
  served();
  const opened: string[] = [];
  const screen = await render(<TaskChecks project="p" head={head} onReference={(ref) => opened.push(ref)} />);

  await expect.element(screen.getByRole('heading', { name: 'Checks failed' })).toBeVisible();
  await expect.element(screen.getByText('8bd41c2', { exact: true })).toBeVisible();
  await expect.element(screen.getByText('npm test exited with 1')).toBeVisible();
  await screen.getByRole('button', { name: 'Reference made-up-ref' }).click();
  expect(opened).toEqual(['made-up-ref']);
  // A closed step's summary line: WebKit's toBeVisible counts all but the <summary> itself as hidden there.
  await expect.element(screen.getByText('Exit 0 · 1m 48s')).toBeInTheDocument();
  await expect.element(screen.getByText('Exit 1 · 27.0 s')).toBeVisible();
  await expect.element(screen.getByText('Expected cursor 42; received 41')).toBeVisible();
  await expect.element(screen.getByText('test result: ok. 36 passed')).not.toBeVisible();
});

test('a run at the commit shows as it goes: none yet, running, then passed when its live event comes', async () => {
  runs = [];
  served();
  const screen = await render(<TaskChecks project="p" head={head} onReference={() => {}} />);
  await expect.element(screen.getByText('No checks run at this commit yet')).toBeVisible();

  runs = [run({ state: 'running', ended: null, steps: [] })];
  for (const on of live) on({ type: 'checks', project: 'p', head, run: 'r1', state: 'running' });
  await expect.element(page.getByRole('heading', { name: 'Checks running' })).toBeVisible();

  runs = [run({})];
  // Another project's run changes nothing here.
  for (const on of live) on({ type: 'checks', project: 'q', head, run: 'x', state: 'failed' });
  for (const on of live) on({ type: 'checks', project: 'p', head, run: 'r1', state: 'passed' });
  await expect.element(page.getByRole('heading', { name: 'Checks passed' })).toBeVisible();
  await expect.element(page.getByText('2 commands · 2m 15s')).toBeVisible();
});

test("a refused load says why; the next live event's load clears it", async () => {
  runs = [run({})];
  refuse = true;
  served();
  const screen = await render(<TaskChecks project="p" head={head} onReference={() => {}} />);
  await expect.element(screen.getByText('made-up refusal')).toBeVisible();
  refuse = false;
  for (const on of live) on({ type: 'checks', project: 'p', head, run: 'r1', state: 'passed' });
  await expect.element(page.getByRole('heading', { name: 'Checks passed' })).toBeVisible();
  await expect.element(page.getByText('made-up refusal')).not.toBeInTheDocument();
});
