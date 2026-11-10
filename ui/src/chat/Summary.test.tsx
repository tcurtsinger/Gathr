import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import '../theme.css';
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
