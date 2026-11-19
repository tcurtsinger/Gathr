// A team chat as main messages, each with its thread (0075), drawn from the service's own answers as its tests recorded
// them: what is a main message and in what order, the fold line, long threads, the result of an ask, a chat of one
// agent, and a search hit in a folded thread. Pointer and keyboard.
import { expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message, Turn } from '../api';
import { chat, View } from './recorded';
import { apply, type ChatState } from './state';

/** The main messages drawn, top to bottom: who speaks. */
const mains = () =>
  [...document.querySelectorAll('.thread-messages > .said:not([data-reply])')].map(
    (el) => el.querySelector('.name-line-name')?.textContent,
  );
const replies = () => [...document.querySelectorAll('.thread-messages > .said[data-reply]')];
/** A main message's fold line: in the row under its bubble (0076). */
const foldOf = (main: Element) => main.querySelector<HTMLButtonElement>('.said-foot .fold-line');
const mainWith = (words: string) =>
  [...document.querySelectorAll('.thread-messages > .said:not([data-reply])')].find((el) =>
    el.textContent?.includes(words),
  ) as HTMLElement;

/** A line of Gathr's in a thread, said in no turn, as the live socket sends it with its topic. */
const line = (id: string, topic: string, body: string, time: string): Message => ({
  id,
  turn: null,
  time,
  role: 'gathr',
  kind: 'text',
  body,
  topic,
});

test('the main line reads the ask, its plan and the tasks handed out, in order', async () => {
  await render(<View state={chat('askAtWork')} />);
  await expect.element(page.getByText('Add the tags table')).toBeVisible();
  expect(mains()).toEqual(['You', 'Lead', 'Lead', 'Lead']);
  // The plan was decided when the chat opened: its thread starts folded. The ask is still at work: open.
  expect(foldOf(mainWith('version 1'))?.getAttribute('aria-expanded')).toBe('false');
  expect(foldOf(mainWith('Add tags to notes'))?.getAttribute('aria-expanded')).toBe('true');
});

test("a task's message opens More to its brief by keyboard too, and its title opens the task", async () => {
  const openTask = vi.fn();
  await render(<View state={chat('askAtWork')} actions={{ openTask }} />);
  const task = mainWith('Add the tags table');
  const more = task.querySelector<HTMLButtonElement>('.task-main-more') as HTMLButtonElement;
  expect(task.textContent).toContain('Notes need tags');
  expect(task.textContent).not.toContain('A note carries its tags');
  more.focus();
  await userEvent.keyboard('{Enter}');
  await expect.element(page.getByText('A note carries its tags')).toBeVisible();
  expect(more.getAttribute('aria-expanded')).toBe('true');
  await page.getByRole('button', { name: 'Add the tags table' }).click();
  expect(openTask).toHaveBeenCalledWith(2);
});

test("a review and the fix after it read in the work task's thread, never on the main line", async () => {
  await render(<View state={chat('reviewAndFix')} />);
  await expect.element(page.getByText('Add the feature')).toBeVisible();
  expect(mains()).toEqual(['You', 'Lead']);
  const vale = replies().find((r) => r.querySelector('.name-line-name')?.textContent === 'Vale');
  expect(vale).toBeDefined();
  // After the task's own message: in its thread.
  const task = mainWith('Add the feature');
  expect(task.compareDocumentPosition(vale as Element) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});

test('the result is at the bottom quoting its ask, and the quote leads to the ask', async () => {
  await render(<View state={chat('askDone')} />);
  const all = document.querySelectorAll('.thread-messages > .said:not([data-reply])');
  const result = all[all.length - 1] as HTMLElement;
  await expect.element(page.getByRole('button', { name: 'Add tags to notes' })).toBeVisible();
  expect(result.querySelector('.bubble-quote')?.textContent).toBe('Add tags to notes');
  const ask = mainWith('Add tags to notes');
  await page.getByRole('button', { name: 'Add tags to notes' }).click();
  await expect.poll(() => ask.hasAttribute('data-search-hit')).toBe(true);
});

/** The done task's thread with `n` more lines of Gathr's in it, a minute apart. */
function longThread(n: number): ChatState {
  const state = chat('askDone');
  const lines = Array.from({ length: n }, (_, i) =>
    line(`l${i}`, 't:2', `Made-up note ${i + 1} on the tags table`, `2026-11-18T20:${10 + i}:00Z`),
  );
  return { ...state, messages: [...state.messages, ...lines] };
}

test('a long thread shows its newest three under "Show N earlier replies", by keyboard too', async () => {
  // Ann's block and seven notes: eight replies.
  await render(<View state={longThread(7)} />);
  const fold = foldOf(mainWith('Add the tags table')) as HTMLButtonElement;
  expect(fold.textContent).toContain('8 replies');
  await fold.click();
  const earlier = page.getByRole('button', { name: 'Show 5 earlier replies' });
  await expect.element(earlier).toBeVisible();
  for (const i of [5, 6, 7])
    await expect.element(page.getByText(`Made-up note ${i} on the tags table`)).toBeVisible();
  expect(document.body.textContent).not.toContain('Made-up note 4 on');
  (earlier.element() as HTMLButtonElement).focus();
  await userEvent.keyboard('{Enter}');
  await expect.element(page.getByText('Made-up note 1 on the tags table')).toBeVisible();
  // Focus goes to the first reply that showed, where the button was.
  await expect.poll(() => document.activeElement?.classList.contains('said')).toBe(true);
});

test('a reply holding an approval waiting is never behind "Show earlier"', async () => {
  const state = longThread(7);
  const ann = Object.values(state.turns).find((t) => t.topic === 't:2')?.id as string;
  state.turns[ann] = { ...state.turns[ann], status: 'running' };
  state.approvals = [{ turn: ann, request: 'r1', tool: 'Bash', input: { command: 'cargo test' } }];
  await render(<View state={state} />);
  await expect.element(page.getByRole('button', { name: 'Show 4 earlier replies' })).toBeVisible();
  await expect.element(page.getByText('Allow Bash?')).toBeVisible();
});

test('a handled thread starts folded; a new reply never opens it, and the person’s fold holds', async () => {
  const state = chat('askDone');
  const screen = await render(<View state={state} />);
  const fold = () => foldOf(mainWith('Add the tags table')) as HTMLButtonElement;
  await expect.poll(() => fold()?.getAttribute('aria-expanded')).toBe('false');
  const more = apply(state, {
    type: 'message',
    chat: 'c',
    message: line('n1', 't:2', 'Made-up later note', '2026-11-18T21:00:00Z'),
  });
  await screen.rerender(<View state={more} />);
  await expect.poll(() => fold().textContent).toContain('2 replies');
  expect(fold().getAttribute('aria-expanded')).toBe('false');
  await fold().click();
  await expect.element(page.getByText('Made-up later note')).toBeVisible();
  const again = apply(more, {
    type: 'message',
    chat: 'c',
    message: line('n2', 't:2', 'Made-up last note', '2026-11-18T21:01:00Z'),
  });
  await screen.rerender(<View state={again} />);
  await expect.element(page.getByText('Made-up last note')).toBeVisible();
  expect(fold().getAttribute('aria-expanded')).toBe('true');
});

test('the fold line says how many replies and nothing more; open, it says Collapse replies', async () => {
  await render(<View state={longThread(1)} />);
  const fold = () => foldOf(mainWith('Add the tags table')) as HTMLButtonElement;
  await expect.poll(() => fold()?.textContent).toBe('2 replies');
  expect(fold().querySelector('.face, time')).toBeNull();
  await fold().click();
  await expect.poll(() => fold().textContent).toBe('Collapse replies');
  expect(fold().getAttribute('aria-expanded')).toBe('true');
  await fold().click();
  await expect.poll(() => fold().textContent).toBe('2 replies');
});

/** A thumbs-up from the chat's second member on a message. */
const liked = (state: ChatState, message: string): ChatState => ({
  ...state,
  reactions: [{ message, agent: state.members[1].agent, time: '2026-11-18T21:00:00Z' }],
});

test("a main message's thumbs-up sits in the row under its bubble, after the replies button", async () => {
  const state = chat('askAtWork');
  await render(<View state={liked(state, state.plans[0].versions[0].message as string)} />);
  await expect.poll(() => document.querySelector('.said-foot .react')).not.toBeNull();
  const main = mainWith('version 1');
  const row = main.querySelector('.said-foot') as HTMLElement;
  expect(main.querySelectorAll('.reacts')).toHaveLength(1);
  const line = (row.querySelector('.fold-line') as HTMLElement).getBoundingClientRect();
  const react = (row.querySelector('.react') as HTMLElement).getBoundingClientRect();
  expect(react.left).toBeGreaterThan(line.right);
  // One row: their middles are level.
  expect(Math.abs((line.top + line.bottom) / 2 - (react.top + react.bottom) / 2)).toBeLessThan(2);
});

test('a main message with a thumbs-up and no replies shows it with no replies button', async () => {
  // A turn of the lead's that nothing ties: a main message with no thread.
  const state = chat('askDone');
  const lead = state.members[0].agent;
  const lone = {
    id: 'lone',
    agent: lead,
    status: 'done',
    why: null,
    ref: 'lone',
    position: 99,
    topic: 'b:lone',
  };
  const words: Message = {
    id: 'lone-words',
    turn: 'lone',
    time: '2026-11-18T22:00:00Z',
    role: 'assistant',
    kind: 'text',
    body: 'Made-up lone words',
  };
  const more = {
    ...state,
    turns: { ...state.turns, lone: lone as Turn },
    messages: [...state.messages, words],
  };
  await render(<View state={liked(more, 'lone-words')} />);
  await expect.element(page.getByText('Made-up lone words')).toBeVisible();
  const main = mainWith('Made-up lone words');
  const row = main.querySelector('.said-foot') as HTMLElement;
  expect(row.querySelector('.react')).not.toBeNull();
  expect(row.querySelector('.fold-line')).toBeNull();
  // No thread item follows it.
  expect(!!main.nextElementSibling?.matches('[data-reply], [data-thread-item]')).toBe(false);
});

test('a thread whose turn failed is not handled: it opens with Retry and its reference in view', async () => {
  const state = chat('askDone');
  const ann = Object.values(state.turns).find((t) => t.topic === 't:2')?.id as string;
  state.turns[ann] = { ...state.turns[ann], status: 'failed', why: 'the engine exited', ref: 'made0up1' };
  await render(<View state={state} />);
  await expect.element(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  expect(foldOf(mainWith('Add the tags table'))?.getAttribute('aria-expanded')).toBe('true');
});

test('nothing folds by itself while the chat is open: a thread handled since stays as it was', async () => {
  const screen = await render(<View state={chat('askAtWork')} />);
  const fold = () => foldOf(mainWith('Add tags to notes')) as HTMLButtonElement;
  await expect.poll(() => fold()?.getAttribute('aria-expanded')).toBe('true');
  // The same chat once all is in: the ask is done, its thread handled.
  await screen.rerender(<View state={chat('askDone')} />);
  // Its result, said once all is in, is drawn.
  await expect.poll(() => document.querySelector('.bubble-quote')?.textContent).toBe('Add tags to notes');
  expect(fold().getAttribute('aria-expanded')).toBe('true');
});

test('a chat of one agent is one timeline: no fold line', async () => {
  const state = chat('askDone');
  await render(<View state={{ ...state, members: state.members.slice(0, 1) }} />);
  await expect.element(page.getByText('Add tags to notes', { exact: true })).toBeVisible();
  expect(document.querySelectorAll('.fold-line, [data-reply]')).toHaveLength(0);
});

test('what came before the chat became a team stays one timeline; threads begin after', async () => {
  const state = chat('askDone');
  // The second agent joined after the ask and the lead's first turn.
  const since = state.messages.find((m) => m.body === 'Hi!' && m.turn === state.messages[0].turn)
    ?.time as string;
  await render(<View state={{ ...state, teamSince: new Date(Date.parse(since) + 1).toISOString() }} />);
  await expect.element(page.getByText('Add the tags table')).toBeVisible();
  expect(foldOf(mainWith('Add tags to notes'))).toBeNull();
  expect(foldOf(mainWith('Add the tags table'))).not.toBeNull();
});

test('an old chat, tied by task and answers alone, reads as main messages with threads', async () => {
  await render(<View state={chat('oldChat')} />);
  await expect.element(page.getByText('Sort the gravel bars', { exact: true })).toBeVisible();
  // The person's message, the task handed out, and the turn nothing ties, its own main message. Its times are a few
  // microseconds apart, closer than a message's time says, so only which ones are main is checked.
  expect(mains().sort()).toEqual(['Gathr', 'Mira', 'You']);
  // The task is done: its thread starts folded.
  await (foldOf(mainWith('@Rowan')) as HTMLButtonElement).click();
  await expect.element(page.getByText('Sorted, herons first')).toBeVisible();
  const rowan = replies().find((r) => r.textContent?.includes('Sorted, herons first'));
  const task = mainWith('@Rowan');
  expect(task.compareDocumentPosition(rowan as Element) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});

test('a search hit in a folded thread opens the thread at the hit', async () => {
  const state = chat('askDone');
  // Ann's words on the done task: its thread starts folded.
  const ann = state.messages.find(
    (m) => m.turn && state.turns[m.turn]?.topic === 't:2' && m.role === 'assistant',
  );
  await render(<View state={state} found={ann?.id} />);
  await expect.poll(() => document.querySelector('[data-search-hit]')?.textContent ?? '').toContain('Hi!');
  expect(foldOf(mainWith('Add the tags table'))?.getAttribute('aria-expanded')).toBe('true');
});
