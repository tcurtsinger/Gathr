import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Task, Wait } from '../api';
import { Summary } from './Summary';
import { empty } from './state';

const members = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' as const },
  { agent: 'b', name: 'Made-up Member', role: 'member' as const },
  { agent: 'c', name: 'Made-up Third', role: 'member' as const },
];
const at = (d: Date) => d.getTime() * 1000;
const today = (h: number, m: number) => {
  const d = new Date();
  d.setHours(h, m, 0, 0);
  return at(d);
};
const task = (number: number, giver: string, owner: string, created_us: number): Task => ({
  number,
  chat: 'c',
  giver,
  owner,
  given_in: null,
  kind: 'work',
  what: `Made-up task ${number}`,
  state: 'working',
  reason: null,
  after: [],
  created_us,
});
const wait = (agent: string, target: string, created_ts: number): Wait => ({
  id: `w-${agent}`,
  agent,
  task: null,
  kind: 'agent',
  target,
  state: 'open',
  created_ts,
});

test('each named wait in Summary says when it began: the time today, the date before today', async () => {
  const earlier = new Date(2026, 10, 8, 16, 20);
  const state = {
    ...empty,
    members,
    // The lead waits on the member through a wait, and on the third through a task it gave.
    waits: [wait('a', 'b', today(9, 42))],
    tasks: [task(1, 'a', 'c', today(10, 5)), task(2, 'b', 'c', at(earlier))],
  };
  const screen = await render(
    <Summary
      state={state}
      pauses={{ pauses: [], usage: [] }}
      run={{ state: 'running', by: null, scope: null }}
      onPause={() => {}}
    />,
  );
  await screen.getByRole('button', { name: 'Summary' }).click();
  const summary = page.getByRole('dialog', { name: 'Summary' });
  await expect
    .element(summary)
    .toMatchTextContent('Supervisor · Waiting for Made-up Member since 09:42 and Made-up Third since 10:05');
  const day = earlier.toLocaleDateString([], { month: 'short', day: 'numeric' });
  await expect.element(summary).toMatchTextContent(`Member · Waiting for Made-up Third since ${day}`);
});

test("each agent's progress counts its done tasks in this chat, cancelled left out; no tasks, no count", async () => {
  const state = {
    ...empty,
    members,
    tasks: [
      { ...task(1, 'a', 'b', 0), state: 'done' as const },
      { ...task(2, 'a', 'b', 0), state: 'held' as const },
      { ...task(3, 'a', 'b', 0), state: 'cancelled' as const },
      { ...task(4, 'a', 'b', 0), state: 'blocked' as const, reason: 'Made-up fixture missing' },
      { ...task(5, 'a', 'b', 0), state: 'failed' as const, reason: 'Made-up command failed' },
    ],
  };
  const screen = await render(
    <Summary
      state={state}
      pauses={{ pauses: [], usage: [] }}
      run={{ state: 'running', by: null, scope: null }}
      onPause={() => {}}
    />,
  );
  await screen.getByRole('button', { name: 'Summary' }).click();
  const member = page.getByRole('progressbar', { name: 'Made-up Member progress' });
  await expect.element(member).toHaveAttribute('aria-valuetext', '1/4');
  await expect.element(member).toHaveAttribute('aria-valuenow', '25');
  await expect
    .element(page.getByRole('progressbar', { name: 'Made-up Third progress' }))
    .not.toBeInTheDocument();

  await page.getByRole('button', { name: /^Made-up Member/ }).click();
  const items = page.getByRole('dialog', { name: 'Summary' }).getByRole('listitem');
  await expect
    .poll(() => items.all().map((i) => i.element().getAttribute('data-status')))
    .toEqual(['active', 'done', 'pending', 'failed']);
  await expect.element(items.nth(2)).toMatchTextContent('Blocked: Made-up fixture missing');
  await expect.element(items.nth(3)).toMatchTextContent('Made-up command failed');

  await page.getByRole('button', { name: /^Made-up Third/ }).click();
  await expect.element(page.getByText('No assigned tasks')).toBeVisible();
});

test("each agent's context shows beside its name, from the engine's fill and window; none before its first call", async () => {
  const state = {
    ...empty,
    members: [
      { ...members[0], context: { used: 250_000, window: 1_000_000 } },
      { ...members[1], context: null },
    ],
  };
  const screen = await render(
    <Summary
      state={state}
      pauses={{ pauses: [], usage: [] }}
      run={{ state: 'running', by: null, scope: null }}
      onPause={() => {}}
    />,
  );
  await screen.getByRole('button', { name: 'Summary' }).click();
  const bars = page.getByRole('dialog', { name: 'Summary' }).getByRole('button', { name: 'Context usage' });
  await expect.element(bars).toHaveTextContent('250k (25%)');
  expect(bars.elements()).toHaveLength(1);
});
