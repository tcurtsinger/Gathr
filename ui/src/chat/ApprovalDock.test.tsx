// The needs dock (0037) with a plan waiting on the person (0062 H5): after any approval, one at a time.
import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { PlanSummary } from '../api';
import { ApprovalDock } from './ApprovalDock';
import type { Approval } from './state';

const planOf = (id: number, state: PlanSummary['state']): PlanSummary => ({
  id,
  ask: `m${id}`,
  title: 'Tags on notes',
  lead: 'mira',
  state,
  versions: [1, 2].map((n) => ({
    id: 10 * id + n,
    n,
    message: `v${n}`,
    parts: n === 2 ? 9 : 7,
    check_line: null,
    sent: null,
    decided: n === 1 ? ('sent_back' as const) : null,
    decided_line: null,
  })),
});
const approval: Approval = { turn: 't1', request: 'r1', tool: 'Bash', input: { command: 'cargo build' } };

const dock = (approvals: Approval[], onOpenPlan = () => {}) => (
  <ApprovalDock
    approvals={approvals}
    plans={[planOf(2, 'checking'), planOf(3, 'waiting'), planOf(4, 'approved')]}
    name={() => 'Ann'}
    lead={(agent) => (agent === 'mira' ? 'Mira' : 'An agent')}
    onShow={() => {}}
    onAnswer={() => {}}
    onOpenPlan={onOpenPlan}
  />
);

test('a waiting plan with no approval needs the person, and Open opens its newest version', async () => {
  const opened = vi.fn();
  const screen = await render(dock([], opened));
  const needs = screen.getByRole('region', { name: 'Needs you' });
  await expect.element(needs.getByText('Needs you · 1 of 1')).toBeVisible();
  await expect.element(needs.getByText("Mira's plan: Tags on notes · 9 parts")).toBeVisible();
  await needs.getByRole('button', { name: 'Open' }).click();
  expect(opened).toHaveBeenCalledWith(3, 2);
});

test('an approval waiting comes before the plan', async () => {
  const screen = await render(dock([approval]));
  const needs = screen.getByRole('region', { name: 'Needs you' });
  await expect.element(needs.getByText('Needs you · 1 of 2')).toBeVisible();
  expect(needs.element().textContent).toContain('Ann wants to run cargo build');
  await expect.element(needs.getByRole('button', { name: 'Allow' })).toBeVisible();
  await expect.element(needs.getByRole('button', { name: 'Open' })).not.toBeInTheDocument();
});
