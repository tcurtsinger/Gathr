import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Turn } from '../api';
import { type ChatState, empty } from './state';
import { Working, working } from './Working';

const members = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' as const },
  { agent: 'kit', name: 'Kit', role: 'member' as const },
  { agent: 'lane', name: 'Lane Ortiz', role: 'member' as const },
];
const turn = (id: string, agent: string, status: Turn['status'], created?: string): Turn => ({
  id,
  agent,
  status,
  why: null,
  ref: id,
  created,
});
const nineFifteen = Date.UTC(2026, 10, 10, 9, 15);
const iso = (ms: number) => new Date(ms).toISOString();

test('the Working line names every agent with a turn running, since the longest-running one began', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: {
      ask: turn('ask', 'mira', 'done', iso(nineFifteen - 60_000)),
      kit: turn('kit', 'kit', 'running', iso(nineFifteen + 120_000)),
      lane: turn('lane', 'lane', 'running', iso(nineFifteen)),
      next: turn('next', 'mira', 'queued'),
    },
  };
  expect(working(state)).toEqual({ members: [members[1], members[2]], since: nineFifteen });

  const screen = await render(<Working state={state} />);
  await expect.element(screen.getByRole('img', { name: 'Kit and Lane Ortiz' })).toBeVisible();
  expect(screen.getByRole('status').element().textContent).toMatch(/^WorkingKLO· \d/);

  // Nobody running: no line.
  const idle = { ...state, turns: { ask: state.turns.ask } };
  await screen.rerender(<Working state={idle} />);
  expect(screen.getByRole('status').query()).toBeNull();
});
