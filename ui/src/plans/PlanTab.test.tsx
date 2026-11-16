// The plan's tab (0062 H5.3, H5.4), first PR: the plain tab, read from GET /api/plans/{plan}, with Approve,
// Send back and Drop at its foot.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Plan, PlanPart, PlanVersion } from '../api';
import type { LiveEvent } from '../live';
import { PlanTab } from './PlanTab';

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

const names: Record<string, string> = { mira: 'Mira', ann: 'Ann', bo: 'Bo' };
const name = (agent: string | null) => (agent && names[agent]) || 'An agent';

const part = (n: number): PlanPart => ({
  n,
  owner: n % 2 ? 'ann' : 'bo',
  what: `Made-up part ${n}`,
  why: `Why ${n}`,
  done: `Done ${n}`,
  proof: `Proof ${n}`,
  where: `room/tags-${n}`,
  size: `src/tags${n}.rs`,
  lines: n * 10,
  after: n === 2 ? [1] : [],
  after_tasks: n === 2 ? [41] : [],
  task: null,
});
const version = (n: number, more: Partial<PlanVersion> = {}): PlanVersion => ({
  id: 10 + n,
  n,
  message: `v${n}`,
  check_line: `c${n}`,
  sent: '2026-11-15T09:10:00.000Z',
  decided: null,
  decided_line: null,
  title: 'Tags on notes',
  text: 'Tags for each note, kept in their own table.',
  created: '2026-11-15T09:00:00.000Z',
  words: null,
  answers: [],
  check: {
    task: 20 + n,
    by: 'bo',
    state: 'blocked',
    outcome: 'findings',
    notes: [
      { id: 1, kind: 'wrong', file: 'src/notes.rs', says: 'Made-up wrong fact' },
      { id: 2, kind: 'wrong', file: null, says: 'Made-up missed step' },
      { id: 3, kind: 'unasked', file: null, says: 'Made-up extra' },
      { id: 4, kind: 'suggestion', file: null, says: 'Made-up idea' },
    ],
  },
  parts: Array.from({ length: 9 }, (_, i) => part(i + 1)),
  ...more,
});
const plan = (more: Partial<Plan> = {}): Plan => ({
  id: 3,
  ask: 'm1',
  title: 'Tags on notes',
  lead: 'mira',
  state: 'waiting',
  asked: 'Add tags to notes',
  questions: [],
  versions: [version(1, { decided: 'sent_back', decided_line: 'sb' }), version(2)],
  ...more,
});

/** The service: the plan as given, and each POST recorded and answered with `answer`. */
function serve(shown: Plan, answer?: { status: number; body: unknown }) {
  const posted: { path: string; body: unknown }[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST') {
      posted.push({ path, body: JSON.parse(String(init.body)) });
      return new Response(JSON.stringify(answer?.body ?? shown), { status: answer?.status ?? 200 });
    }
    return new Response(JSON.stringify(path === '/api/plans/3' ? shown : {}));
  });
  return posted;
}

const tab = (shownVersion = 2) => (
  <PlanTab chat="c" plan={3} version={shownVersion} name={name} onVersion={() => {}} onReference={() => {}} />
);

test('a waiting plan shows its version, title, the ask, its check and every part whole, in order', async () => {
  serve(plan());
  const screen = await render(tab());
  const page = screen.getByRole('region', { name: 'Plan' });
  await expect.element(page.getByRole('heading', { name: 'Tags on notes' })).toBeVisible();
  const words = page.element().textContent ?? '';
  const order = [
    'Plan · version 2',
    'Tags on notes',
    'You asked',
    'Add tags to notes',
    'Check',
    '3 findings from Bo',
    '9 parts',
  ];
  const at = order.map((w) => words.indexOf(w));
  expect(at.every((x) => x >= 0)).toBe(true);
  expect(at).toEqual([...at].sort((a, b) => a - b));
  const second = page.getByRole('listitem').nth(1);
  for (const line of [
    'Made-up part 2',
    'Bo · after part 1, #41',
    'Why: Why 2',
    'Done when: Done 2',
    'Proof: Proof 2',
    'Where: room/tags-2',
    'Size: src/tags2.rs, about 20 lines',
  ])
    await expect.element(second.getByText(line, { exact: true })).toBeVisible();
  expect(page.getByRole('listitem').elements()).toHaveLength(9);
});

test('Approve sends the version shown', async () => {
  const posted = serve(plan());
  const screen = await render(tab());
  await screen.getByRole('button', { name: 'Approve' }).click();
  await expect.poll(() => posted).toEqual([{ path: '/api/plans/3/approve', body: { version: 2 } }]);
});

test("Send back asks what to change, stays off while it's empty, sends the words; Cancel puts the buttons back", async () => {
  const posted = serve(plan());
  const screen = await render(tab());
  await screen.getByRole('button', { name: 'Send back' }).click();
  const field = screen.getByRole('textbox', { name: /What to change/ });
  await expect.element(field).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
  const send = screen.getByRole('button', { name: 'Send back' });
  await expect.element(send).toBeDisabled();
  await screen.getByRole('button', { name: 'Cancel' }).click();
  await expect.element(field).not.toBeInTheDocument();
  await expect.element(screen.getByRole('button', { name: 'Approve' })).toBeVisible();
  await screen.getByRole('button', { name: 'Send back' }).click();
  await screen.getByRole('textbox', { name: /What to change/ }).fill('Split part 2');
  await expect.element(send).toBeEnabled();
  await send.click();
  await expect
    .poll(() => posted)
    .toEqual([{ path: '/api/plans/3/send-back', body: { version: 2, words: 'Split part 2' } }]);
});

test('a refused Approve shows why with its reference', async () => {
  const refusal = { error: 'Approve failed', why: 'Version 2 is already approved.', ref: 'made-up-ref' };
  serve(plan(), { status: 409, body: refusal });
  const screen = await render(tab());
  await screen.getByRole('button', { name: 'Approve' }).click();
  await expect.element(screen.getByText('Approve failed')).toBeVisible();
  await expect.element(screen.getByText('Version 2 is already approved.')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Reference made-up-ref' })).toBeVisible();
});

test('a refused Approve reads the plan again, and the buttons follow the plan it returns', async () => {
  const refusal = { error: 'Approve failed', why: 'Version 2 is already approved.', ref: 'made-up-ref' };
  let shown = plan();
  let reads = 0;
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST') return new Response(JSON.stringify(refusal), { status: 409 });
    if (path === '/api/plans/3') reads += 1;
    return new Response(JSON.stringify(shown));
  });
  const screen = await render(tab());
  await expect.element(screen.getByRole('button', { name: 'Approve' })).toBeVisible();
  // Approved elsewhere, and this tab missed the live event that said so.
  shown = plan({ state: 'approved' });
  await screen.getByRole('button', { name: 'Approve' }).click();
  await expect.element(screen.getByText('Version 2 is already approved.')).toBeVisible();
  await expect.poll(() => reads).toBe(2);
  await expect.element(screen.getByText('Approved', { exact: true })).toBeVisible();
  for (const label of ['Drop', 'Send back', 'Approve'])
    await expect.element(screen.getByRole('button', { name: label })).not.toBeInTheDocument();
  await expect.element(screen.getByText('Version 2 is already approved.')).toBeVisible();
});

test('a version being checked says Checking, with Drop and no Send back or Approve', async () => {
  const checking = version(2, {
    sent: null,
    check: {
      ...(version(2).check as NonNullable<PlanVersion['check']>),
      state: 'working',
      outcome: null,
      notes: [],
    },
  });
  serve(plan({ state: 'checking', versions: [version(1, { decided: 'sent_back' }), checking] }));
  const screen = await render(tab());
  await expect.element(screen.getByText('Checking', { exact: true })).toBeVisible();
  await expect.element(screen.getByText('Bo is checking')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Drop' })).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Send back' })).not.toBeInTheDocument();
  await expect.element(screen.getByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
});

test("an older version says Replaced, with none of the foot's buttons", async () => {
  serve(plan());
  const screen = await render(tab(1));
  await expect.element(screen.getByText('Plan · version 1')).toBeVisible();
  await expect.element(screen.getByText('Replaced')).toBeVisible();
  for (const label of ['Drop', 'Send back', 'Approve'])
    await expect.element(screen.getByRole('button', { name: label })).not.toBeInTheDocument();
});
