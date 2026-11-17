// The plan's tab (0062 H5.3, H5.4), read from GET /api/plans/{plan}: the check with its Findings, Answers and
// Questions, parts that open one at a time with their marks, and Approve, Send back and Drop at its foot.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Plan, PlanPart, PlanSummary, PlanVersion } from '../api';
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
  changed: [],
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

/** In the workspace at its narrowest, 360px (DESIGN.md Layout), as a container like the workspace's own. */
const narrow = (shown: React.ReactNode) => (
  <div
    style={{
      width: 360,
      height: 700,
      display: 'flex',
      containerType: 'inline-size',
      containerName: 'workspace',
    }}
  >
    {shown}
  </div>
);

/** What doesn't fit in the foot: a part past its edge, two parts over each other, a clipped label, a scroll. */
function footMisfits() {
  const pager = document.querySelector('.tasks-pager') as HTMLElement;
  const box = pager.getBoundingClientRect();
  const parts = [...pager.querySelectorAll('button, [role="status"]')] as HTMLElement[];
  const at = parts.map((p) => {
    const b = p.getBoundingClientRect();
    return {
      what: p.textContent || p.getAttribute('role'),
      left: Math.round(b.left),
      right: Math.round(b.right),
    };
  });
  const wrong = at
    .filter((p) => p.left < box.left - 0.5 || p.right > box.right + 0.5)
    .map((p) => `${p.what} past the edge`);
  for (let i = 1; i < at.length; i++)
    if (at[i].left < at[i - 1].right) wrong.push(`${at[i].what} over ${at[i - 1].what}`);
  for (const p of parts) if (p.scrollWidth > p.clientWidth + 1) wrong.push(`${p.textContent} clipped`);
  if (pager.scrollWidth > pager.clientWidth + 1) wrong.push('the foot scrolls sideways');
  return {
    wrong,
    measured: JSON.stringify({ pager: [Math.round(box.left), Math.round(box.right)], parts: at }),
  };
}

/** A part's brief as the open part shows it: each label and its words, in order. */
const rows = (dl: Element | null) => [...(dl?.children ?? [])].map((e) => e.textContent).join(' | ');

test('a waiting plan shows its version, title, the ask, its check and every part, in order', async () => {
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
  expect(page.getByRole('listitem').elements()).toHaveLength(9);
  const second = page.getByRole('listitem').nth(1);
  await second.getByRole('button', { name: 'Made-up part 2' }).click();
  await expect.element(second.getByText('Bo', { exact: true })).toBeVisible();
  expect(rows(second.element().querySelector('dl'))).toBe(
    'Why | Why 2 | Done when | Done 2 | Proof | Proof 2 | Where | room/tags-2 | Size | src/tags2.rs, about 20 lines' +
      ' | After | part 1, #41',
  );
});

test('a click opens one part to its fields and closes the one open', async () => {
  serve(plan());
  const screen = await render(tab());
  const [second, third] = [1, 2].map((i) => screen.getByRole('listitem').nth(i));
  await expect.element(second.getByText('Why 2')).not.toBeInTheDocument();
  await second.getByRole('button', { name: 'Made-up part 2' }).click();
  await expect.element(second.getByText('Why 2')).toBeVisible();
  await expect
    .element(second.getByRole('button', { name: 'Made-up part 2' }))
    .toHaveAttribute('aria-expanded', 'true');
  await third.getByRole('button', { name: 'Made-up part 3' }).click();
  await expect.element(third.getByText('Why 3')).toBeVisible();
  await expect.element(second.getByText('Why 2')).not.toBeInTheDocument();
});

test('choosing another version closes the open part', async () => {
  serve(plan());
  const screen = await render(tab(2));
  await screen.getByRole('button', { name: 'Made-up part 2' }).click();
  await expect.element(screen.getByText('Why 2')).toBeVisible();
  await screen.rerender(tab(1));
  await expect.element(screen.getByText('Plan · version 1')).toBeVisible();
  expect(screen.getByRole('button', { expanded: true }).elements()).toHaveLength(0);
});

test("Check gains the version's Findings, the Answers to the version before's, and the ask's Questions", async () => {
  const before = version(1, { decided: 'sent_back' });
  const now = version(2, {
    answers: [
      { note: 1, changed: true, why: 'Moved the table' },
      { note: 3, changed: false, why: 'The ask needs it' },
    ],
    check: {
      task: 22,
      by: 'bo',
      state: 'blocked',
      outcome: 'findings',
      notes: [
        { id: 5, kind: 'unasked', file: 'src/tags.rs', says: 'Made-up new extra' },
        { id: 6, kind: 'suggestion', file: null, says: 'Made-up second idea' },
      ],
    },
  });
  const questions = [
    { number: 7, what: 'Which notes get tags?', answer: 'All of them' },
    { number: 8, what: 'Any colours?', answer: null },
  ];
  serve(plan({ versions: [before, now], questions }));
  const screen = await render(tab());
  await expect.element(screen.getByText('1 finding from Bo')).toBeVisible();
  const fields = screen.getByRole('region', { name: 'Plan' }).element().querySelector('dl.task-fields');
  expect(rows(fields)).toBe(
    [
      'Check',
      '1 finding from Bo',
      'Findings',
      'Unasked · src/tags.rs: Made-up new extra',
      'Answers',
      'Wrong · src/notes.rs: Made-up wrong factChanged: Moved the table',
      'Unasked · Made-up extraKept: The ask needs it',
      'Questions',
      'Which notes get tags?All of them',
      'Any colours?Open',
    ].join(' | '),
  );
});

test("Changed lists each caught turn's agent and its files", async () => {
  const changed = [
    { agent: 'ann', files: ['src/notes.rs', 'src/tags.rs'], message: 'g1' },
    { agent: 'bo', files: ['new.txt'], message: 'g2' },
  ];
  serve(plan({ changed }));
  const screen = await render(tab());
  await expect.element(screen.getByText('3 findings from Bo')).toBeVisible();
  const fields = screen.getByRole('region', { name: 'Plan' }).element().querySelector('dl.task-fields');
  expect(rows(fields)).toContain('Changed | Ann: src/notes.rs, src/tags.rs | Bo: new.txt');
});

test('Changed names 20 files of a catch and counts the rest, as the chat line does', async () => {
  const many = Array.from({ length: 25 }, (_, i) => `f${String(i + 1).padStart(2, '0')}.txt`);
  const changed = [
    { agent: 'ann', files: many, message: 'g1' },
    { agent: 'bo', files: ['a.txt', 'b.txt'], message: 'g2' },
  ];
  serve(plan({ changed }));
  const screen = await render(tab());
  await expect.element(screen.getByText('3 findings from Bo')).toBeVisible();
  const fields = screen.getByRole('region', { name: 'Plan' }).element().querySelector('dl.task-fields');
  const named =
    'f01.txt, f02.txt, f03.txt, f04.txt, f05.txt, f06.txt, f07.txt, f08.txt, f09.txt, f10.txt, ' +
    'f11.txt, f12.txt, f13.txt, f14.txt, f15.txt, f16.txt, f17.txt, f18.txt, f19.txt, f20.txt';
  expect(rows(fields).split('Changed | ')[1]).toBe(`Ann: ${named} and 5 more | Bo: a.txt, b.txt`);
});

test('a plan with nothing caught has no Changed', async () => {
  serve(plan());
  const screen = await render(tab());
  await expect.element(screen.getByText('3 findings from Bo')).toBeVisible();
  expect(screen.getByText('Changed', { exact: true }).elements()).toHaveLength(0);
});

test('the parts carry Changed, New and Removed against the version before, and an open part its Was', async () => {
  const older = Array.from({ length: 3 }, (_, i) => part(i + 1));
  const newer = [
    part(1),
    { ...part(2), done: 'Done 2, with tests' },
    { ...part(3), what: 'Made-up part 3, split' },
  ];
  serve(
    plan({
      versions: [version(1, { decided: 'sent_back', parts: older }), version(2, { parts: newer })],
    }),
  );
  const screen = await render(tab());
  const items = screen.getByRole('listitem');
  await expect
    .element(items.nth(0).getByRole('button', { name: 'Made-up part 1', exact: true }))
    .toBeVisible();
  await expect.element(items.nth(1).getByRole('button', { name: 'Made-up part 2 Changed' })).toBeVisible();
  await expect.element(items.nth(2).getByRole('button', { name: 'Made-up part 3, split New' })).toBeVisible();
  await expect.element(items.nth(3)).toHaveTextContent('Removed Made-up part 3');
  await items.nth(1).getByRole('button', { name: 'Made-up part 2 Changed' }).click();
  expect(rows(items.nth(1).element().querySelector('dl'))).toContain(
    'Done when | Done 2, with tests | Was | Done 2 | Proof',
  );
  // The first version has none before it, so no marks.
  await screen.rerender(tab(1));
  await expect.element(screen.getByRole('button', { name: 'Made-up part 2', exact: true })).toBeVisible();
  expect(screen.getByText('Removed').elements()).toHaveLength(0);
});

test("the tab's dot follows whether the plan waits on the person", async () => {
  const waits = vi.fn();
  serve(plan());
  await render(
    <PlanTab
      chat="c"
      plan={3}
      version={2}
      name={name}
      onVersion={() => {}}
      onReference={() => {}}
      onWaits={waits}
    />,
  );
  await expect.poll(() => waits.mock.lastCall).toEqual([true]);
  serve(plan({ state: 'approved' }));
  const approved = { id: 3, state: 'approved' } as PlanSummary;
  for (const on of live) on({ type: 'tasks', chat: 'c', tasks: [], waits: [], asks: [], plans: [approved] });
  await expect.poll(() => waits.mock.lastCall).toEqual([false]);
});

test('Approve sends the version shown', async () => {
  const posted = serve(plan());
  const screen = await render(tab());
  await screen.getByRole('button', { name: 'Approve' }).click();
  await expect.poll(() => posted).toEqual([{ path: '/api/plans/3/approve', body: { version: 2 } }]);
});

test("Send back asks what to change, stays off while it's empty, sends the words; Cancel puts the buttons back", async () => {
  const posted = serve(plan());
  const screen = await render(narrow(tab()));
  await expect.element(screen.getByRole('button', { name: 'Approve' })).toBeVisible();
  // Both feet fit the workspace at its narrowest, 360px.
  const fits = () => expect(footMisfits().wrong, footMisfits().measured).toEqual([]);
  fits();
  await screen.getByRole('button', { name: 'Send back' }).click();
  const field = screen.getByRole('textbox', { name: /What to change/ });
  await expect.element(field).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
  fits();
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
