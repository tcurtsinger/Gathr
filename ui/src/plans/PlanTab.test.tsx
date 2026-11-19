// The plan's tab (0062 H5.3, H5.4, the plan-tab mock-up, New), read from GET /api/plans/{plan}: the check with its
// Findings, Answers and Questions, each part a Tasks row that opens to its brief, the tasks they became once
// approved, and Drop, Send back and Approve at its foot. Made-up team and plan.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Plan, PlanPart, PlanSummary, PlanVersion, Task } from '../api';
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

const tab = (shownVersion = 2, more: { tasks?: Task[]; onOpenTask?: (task: number) => void } = {}) => (
  <PlanTab
    chat="c"
    plan={3}
    version={shownVersion}
    name={name}
    tasks={more.tasks ?? []}
    onOpenTask={more.onOpenTask ?? (() => {})}
    onVersion={() => {}}
    onReference={() => {}}
  />
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

/** What doesn't fit in the foot: a button past its edge, two buttons over each other, a clipped label. */
function footMisfits() {
  const foot = document.querySelector('footer') as HTMLElement;
  const box = foot.getBoundingClientRect();
  const at = ([...foot.querySelectorAll('button, textarea')] as HTMLElement[]).map((p) => {
    const b = p.getBoundingClientRect();
    const what = p.textContent || p.getAttribute('aria-label');
    return {
      what,
      left: Math.round(b.left),
      right: Math.round(b.right),
      top: Math.round(b.top),
      clipped: p.scrollWidth > p.clientWidth + 1,
    };
  });
  const wrong = at
    .filter((p) => p.left < box.left - 0.5 || p.right > box.right + 0.5)
    .map((p) => `${p.what} past the edge`);
  for (let i = 1; i < at.length; i++)
    if (at[i].top === at[i - 1].top && at[i].left < at[i - 1].right)
      wrong.push(`${at[i].what} over ${at[i - 1].what}`);
  for (const p of at) if (p.clipped) wrong.push(`${p.what} clipped`);
  if (foot.scrollWidth > foot.clientWidth + 1) wrong.push('the foot scrolls sideways');
  return { wrong, measured: JSON.stringify({ foot: [Math.round(box.left), Math.round(box.right)], at }) };
}

/** A brief as the open part shows it: each label and its words, in order. */
const rows = (dl: Element | null) => [...(dl?.children ?? [])].map((e) => e.textContent).join(' | ');
/** Each part's line, in order: "Part 2·Bo·After 1, #41". */
const lines = () => [...document.querySelectorAll('.task-row-line')].map((l) => l.textContent);

test('a waiting plan shows its version, title, its state, the ask, its check and every part, in order', async () => {
  serve(plan());
  const screen = await render(tab());
  const page = screen.getByRole('region', { name: 'Plan' });
  await expect.element(page.getByRole('heading', { name: 'Tags on notes' })).toBeVisible();
  const words = page.element().textContent ?? '';
  const order = [
    'Version 2',
    'Tags on notes',
    'Mira',
    'Waiting',
    'Asked',
    'Add tags to notes',
    'Check',
    '3 findings from Bo',
    'Parts9',
  ];
  const at = order.map((w) => words.indexOf(w));
  expect(
    at.every((x) => x >= 0),
    JSON.stringify(at),
  ).toBe(true);
  expect(at).toEqual([...at].sort((a, b) => a - b));
  expect(page.getByRole('listitem').elements()).toHaveLength(9);
  expect(lines().slice(0, 3)).toEqual(['Part 1·Ann', 'Part 2·Bo·After 1, #41', 'Part 3·Ann']);
  const second = page.getByRole('listitem').nth(1);
  await second.getByRole('button').click();
  expect(rows(second.element().querySelector('dl'))).toBe(
    'Why | Why 2 | Done when | Done 2 | Proof | Proof 2 | Where | room/tags-2 | Size | src/tags2.rs, about 20 lines' +
      ' | After | Part 1, #41',
  );
});

test('a click opens one part to its brief and closes the one open', async () => {
  serve(plan());
  const screen = await render(tab());
  const [second, third] = [1, 2].map((i) => screen.getByRole('listitem').nth(i));
  await expect.element(second.getByText('Why 2')).not.toBeInTheDocument();
  await second.getByRole('button').click();
  await expect.element(second.getByText('Why 2')).toBeVisible();
  await expect.element(second.getByRole('button')).toHaveAttribute('aria-expanded', 'true');
  await third.getByRole('button').click();
  await expect.element(third.getByText('Why 3')).toBeVisible();
  await expect.element(second.getByText('Why 2')).not.toBeInTheDocument();
});

test('choosing another version closes the open part', async () => {
  serve(plan());
  const screen = await render(tab(2));
  await screen.getByRole('listitem').nth(1).getByRole('button').click();
  await expect.element(screen.getByText('Why 2')).toBeVisible();
  await screen.rerender(tab(1));
  await expect.element(screen.getByText('Version 1')).toBeVisible();
  expect(screen.getByRole('button', { expanded: true }).elements()).toHaveLength(0);
});

/** The tab's scroller, scrolled to `top` (measured back, so a page too short to scroll fails here). */
function scrolledTo(top: number) {
  const scroller = document.querySelector('.tasks-scroll') as HTMLElement;
  scroller.scrollTop = top;
  expect(scroller.scrollTop, 'the page scrolls at 700px').toBe(top);
  return scroller;
}

test('another version opens at its top', async () => {
  serve(plan());
  const screen = await render(narrow(tab(2)));
  await expect.element(screen.getByRole('heading', { name: 'Tags on notes' })).toBeVisible();
  const scroller = scrolledTo(240);
  await screen.rerender(narrow(tab(1)));
  await expect.element(screen.getByText('Replaced', { exact: true })).toBeVisible();
  await expect.poll(() => scroller.scrollTop).toBe(0);
});

test('a check that ends while the person reads keeps their place', async () => {
  const checking = version(2, {
    sent: null,
    check: {
      ...(version(2).check as NonNullable<PlanVersion['check']>),
      state: 'working',
      outcome: null,
      notes: [],
    },
  });
  const first = version(1, { decided: 'sent_back' });
  serve(plan({ state: 'checking', versions: [first, checking] }));
  const screen = await render(narrow(tab()));
  await expect.element(screen.getByText('Checking', { exact: true })).toBeVisible();
  const scroller = scrolledTo(100);
  serve(plan({ versions: [first, { ...checking, sent: '2026-11-15T09:10:00.000Z' }] }));
  const waiting = { id: 3, state: 'waiting' } as PlanSummary;
  for (const on of live) on({ type: 'tasks', chat: 'c', tasks: [], waits: [], asks: [], plans: [waiting] });
  await expect.element(screen.getByRole('button', { name: 'Approve' })).toBeVisible();
  expect(scroller.scrollTop).toBe(100);
});

test("the person's own Send back, from a scrolled tab, opens it at its top", async () => {
  serve(plan());
  const screen = await render(narrow(tab()));
  await expect.element(screen.getByRole('button', { name: 'Approve' })).toBeVisible();
  await screen.getByRole('button', { name: 'Send back' }).click();
  await screen.getByRole('textbox', { name: 'What to change' }).fill('Split part 2');
  const scroller = scrolledTo(240);
  serve(plan({ state: 'revising' }));
  await screen.getByRole('button', { name: 'Send back' }).click();
  await expect.element(screen.getByText('Revising', { exact: true })).toBeVisible();
  await expect.poll(() => scroller.scrollTop).toBe(0);
});

test("Check gains the version's Findings, the Answers to the version before's, the ask's Questions and Sent back", async () => {
  const before = version(1, { decided: 'sent_back', words: 'Split the table first' });
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
  const fields = () => screen.getByRole('region', { name: 'Plan' }).element().querySelector('dl.task-fields');
  expect(rows(fields())).toBe(
    [
      'Asked',
      'Add tags to notes',
      'Check',
      '1 finding from Bo',
      'Findings',
      'Not asked for · src/tags.rs: Made-up new extra',
      'Answers',
      'Wrong · src/notes.rs: Made-up wrong factChanged: Moved the table',
      'Not asked for · Made-up extraKept: The ask needs it',
      'Questions',
      'Which notes get tags?All of them',
      'Any colours?Open',
    ].join(' | '),
  );
  await screen.rerender(tab(1));
  await expect.element(screen.getByText('Version 1')).toBeVisible();
  expect(rows(fields())).toContain('Sent back | Split the table first');
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

test("Changed, New and Removed are words on the part's line, against the version before", async () => {
  const older = Array.from({ length: 3 }, (_, i) => part(i + 1));
  const newer = [
    part(1),
    { ...part(2), done: 'Done 2, with tests' },
    { ...part(3), what: 'Made-up part 3, split' },
  ];
  serve(
    plan({ versions: [version(1, { decided: 'sent_back', parts: older }), version(2, { parts: newer })] }),
  );
  const screen = await render(tab());
  await expect.element(screen.getByRole('heading', { name: 'Tags on notes' })).toBeVisible();
  expect(lines()).toEqual(['Part 1·Ann', 'Part 2·Bo·After 1, #41·Changed', 'Part 3·Ann·New', 'Ann·Removed']);
  expect(screen.getByRole('listitem').nth(3).element().querySelector('s')?.textContent).toBe(
    'Made-up part 3',
  );
  // The first version has none before it, so no marks.
  await screen.rerender(tab(1));
  await expect.element(screen.getByText('Version 1')).toBeVisible();
  expect(lines()).toEqual(['Part 1·Ann', 'Part 2·Bo·After 1, #41', 'Part 3·Ann']);
});

test('a Was sits under the field it belongs to, a changed owner has its Owner row, and a part that went is named', async () => {
  const older = [part(1), part(2), part(3), { ...part(4), after: [3] }];
  const newer = [
    part(1),
    { ...part(2), owner: 'ann', done: 'Done 2, with tests' },
    { ...part(3), what: 'Made-up part 3, split' },
    { ...part(4), after: [3] },
  ];
  serve(
    plan({ versions: [version(1, { decided: 'sent_back', parts: older }), version(2, { parts: newer })] }),
  );
  const screen = await render(tab());
  const [second, fourth] = [1, 3].map((i) => screen.getByRole('listitem').nth(i));
  await second.getByRole('button').click();
  expect(rows(second.element().querySelector('dl'))).toBe(
    'Owner | AnnWas: Bo | Why | Why 2 | Done when | Done 2, with testsWas: Done 2 | Proof | Proof 2 | Where | room/tags-2' +
      ' | Size | src/tags2.rs, about 20 lines | After | Part 1, #41',
  );
  // Each Was is in its field's words, under them and in their column.
  for (const was of second.element().querySelectorAll('.plan-was')) {
    const dd = was.closest('dd') as HTMLElement;
    const words = dd.firstChild as HTMLElement;
    const [w, b] = [was.getBoundingClientRect(), words.getBoundingClientRect()];
    expect(dd.previousElementSibling?.tagName, was.textContent ?? '').toBe('DT');
    expect(Math.round(w.top - b.bottom), `${was.textContent} below its words`).toBeGreaterThanOrEqual(0);
    expect(Math.round(w.left - dd.getBoundingClientRect().left), `${was.textContent} in its column`).toBe(0);
  }
  await fourth.getByRole('button').click();
  expect(rows(fourth.element().querySelector('dl'))).toContain('After | Part 3Was: Made-up part 3, removed');
});

const task = (number: number, owner: string, state: Task['state'], what: string): Task => ({
  number,
  chat: 'c',
  giver: 'mira',
  owner,
  given_in: 't1',
  kind: 'work',
  what,
  state,
  reason: null,
  after: [],
  created_us: 0,
});

test('once approved, each part is its task’s row, which opens the task, and the tab has no foot', async () => {
  const handed = version(2, {
    decided: 'approved',
    parts: [{ ...part(1), task: 41 }, { ...part(2), task: 42 }, part(3)],
  });
  const first = version(1, { decided: 'sent_back', parts: [part(1), part(2), part(3)] });
  serve(plan({ state: 'approved', versions: [first, handed] }));
  const tasks = [task(41, 'ann', 'done', 'Made-up part 1'), task(42, 'bo', 'working', 'Made-up part 2')];
  const opened: number[] = [];
  const screen = await render(tab(2, { tasks, onOpenTask: (n) => opened.push(n) }));
  await expect.element(screen.getByText('Approved', { exact: true })).toBeVisible();
  await screen.getByRole('listitem').nth(1).getByRole('button').click();
  expect(opened).toEqual([42]);
  // A part with no task keeps its part row.
  expect(lines()).toEqual(['#41·Ann', '#42·Bo·Working', 'Part 3·Ann']);
  expect(document.querySelector('footer')).toBeNull();
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
      tasks={[]}
      onOpenTask={() => {}}
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

test("Send back opens the answer form's field, stays off while it's empty, sends the words; Cancel puts the buttons back", async () => {
  const posted = serve(plan());
  const screen = await render(narrow(tab()));
  await expect.element(screen.getByRole('button', { name: 'Approve' })).toBeVisible();
  // Both feet fit the workspace at its narrowest, 360px.
  const fits = () => expect(footMisfits().wrong, footMisfits().measured).toEqual([]);
  fits();
  await screen.getByRole('button', { name: 'Send back' }).click();
  const field = screen.getByRole('textbox', { name: 'What to change' });
  await expect.element(field).toHaveFocus();
  await expect.element(field).toHaveAttribute('placeholder', 'What to change');
  await expect.element(screen.getByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
  fits();
  const send = screen.getByRole('button', { name: 'Send back' });
  await expect.element(send).toBeDisabled();
  await screen.getByRole('button', { name: 'Cancel' }).click();
  await expect.element(field).not.toBeInTheDocument();
  await expect.element(screen.getByRole('button', { name: 'Approve' })).toBeVisible();
  await screen.getByRole('button', { name: 'Send back' }).click();
  await screen.getByRole('textbox', { name: 'What to change' }).fill('Split part 2');
  await expect.element(send).toBeEnabled();
  await send.click();
  await expect
    .poll(() => posted)
    .toEqual([{ path: '/api/plans/3/send-back', body: { version: 2, words: 'Split part 2' } }]);
});

test('a refused Approve says Not approved, why and its reference in the foot', async () => {
  const refusal = { error: 'Approve failed', why: 'Version 2 is already approved.', ref: 'made-up-ref' };
  serve(plan(), { status: 409, body: refusal });
  const refs: string[] = [];
  const screen = await render(
    <PlanTab
      chat="c"
      plan={3}
      version={2}
      name={name}
      tasks={[]}
      onOpenTask={() => {}}
      onVersion={() => {}}
      onReference={(r) => refs.push(r)}
    />,
  );
  await screen.getByRole('button', { name: 'Approve' }).click();
  await expect.element(screen.getByText('Not approved')).toBeVisible();
  expect(document.querySelector('footer')?.textContent).toContain(
    'Not approved·Version 2 is already approved.made-up-ref',
  );
  await screen.getByRole('button', { name: 'made-up-ref' }).click();
  expect(refs).toEqual(['made-up-ref']);
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

test('an older version says Replaced, with no foot', async () => {
  serve(plan());
  const screen = await render(tab(1));
  await expect.element(screen.getByText('Version 1')).toBeVisible();
  await expect.element(screen.getByText('Replaced')).toBeVisible();
  expect(document.querySelector('footer')).toBeNull();
});

// The owner, 2026-11-19, with a plan's tab full of raw asterisks and backticks: "We need the same formatting that is
// in the chat in the tasks / plans".
test("a plan's text and a part's brief are formatted as an agent's words in the chat are", async () => {
  const text =
    'Five PRs.\n\n- **Hear-backs.** The line in `tasks/words.rs` keeps its note.\n- **CI failures.** Two.';
  const parts = [{ ...part(1), why: 'Read `waits.rs` **first**.' }];
  serve(plan({ versions: [version(2, { text, parts })] }));
  const screen = await render(tab());
  const page = screen.getByRole('region', { name: 'Plan' });
  await expect.element(page.getByText('Hear-backs.', { exact: true })).toBeVisible();
  const body = page.element().querySelector('.plan-text') as HTMLElement;
  expect(body.textContent).not.toContain('**');
  expect(body.textContent).not.toContain('`');
  expect(
    [...body.querySelectorAll('[data-streamdown="list-item"] [data-streamdown="strong"]')].map(
      (s) => s.textContent,
    ),
  ).toEqual(['Hear-backs.', 'CI failures.']);
  expect(body.querySelector('[data-streamdown="inline-code"]')?.textContent).toBe('tasks/words.rs');

  await page.getByRole('button', { name: /Part 1/ }).click();
  const brief = page
    .element()
    .querySelector('dl.task-fields:has([data-streamdown="inline-code"])') as HTMLElement;
  expect(brief.querySelector('[data-streamdown="inline-code"]')?.textContent).toBe('waits.rs');
  expect(brief.querySelector('[data-streamdown="strong"]')?.textContent).toBe('first');
});
