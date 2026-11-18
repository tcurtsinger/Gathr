// A team chat as main messages, each with its thread (0075), drawn from the service's own answers as its tests recorded
// them: what is a main message and in what order, the state word on each, the fold line, long threads, the result of
// an ask, a chat of one agent, and a search hit in a folded thread. Pointer and keyboard.
import { expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message } from '../api';
import { chat, View } from './recorded';
import { apply, type ChatState } from './state';

/** The main messages drawn, top to bottom: who speaks and the state at the right of the name line. */
const mains = () =>
  [...document.querySelectorAll('.thread-messages > .said:not([data-reply])')].map((el) => [
    el.querySelector('.name-line-name')?.textContent,
    el.querySelector('.main-state-word')?.textContent ?? '',
  ]);
const replies = () => [...document.querySelectorAll('.thread-messages > .said[data-reply]')];
const foldOf = (main: Element) => main.nextElementSibling?.querySelector<HTMLButtonElement>('.fold-line');
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

test('the main line reads the ask, its plan and the tasks handed out, in order, each with its state', async () => {
  await render(<View state={chat('askAtWork')} />);
  await expect.element(page.getByText('Add the tags table')).toBeVisible();
  expect(mains()).toEqual([
    ['You', 'Working'],
    ['Lead', 'Approved'],
    ['Lead', 'Working'],
    ['Lead', 'Not started'],
  ]);
  // The plan was decided when the chat opened: its thread starts folded. The ask is still at work: open.
  expect(foldOf(mainWith('version 1'))?.getAttribute('aria-expanded')).toBe('false');
  expect(foldOf(mainWith('Add tags to notes'))?.getAttribute('aria-expanded')).toBe('true');
});

test('a question waiting on the person is a main message that says Needs you in the attention blue', async () => {
  await render(<View state={chat('questionDiscussed')} />);
  const word = page.getByText('Needs you', { exact: true });
  await expect.element(word).toBeVisible();
  const blue = getComputedStyle(document.documentElement).getPropertyValue('--attention').trim();
  const probe = document.createElement('span');
  probe.style.color = blue;
  document.body.append(probe);
  expect(getComputedStyle(word.element()).color).toBe(getComputedStyle(probe).color);
  probe.remove();
  expect(mains().map(([, state]) => state)).toContain('Needs you');
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
  expect(mains().map(([who]) => who)).toEqual(['You', 'Lead']);
  const vale = replies().find((r) => r.querySelector('.name-line-name')?.textContent === 'Vale');
  expect(vale).toBeDefined();
  // After the task's own fold line: in its thread.
  const fold = mainWith('Add the feature').nextElementSibling as Element;
  expect(fold.compareDocumentPosition(vale as Element) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});

test("a reply on another task than its thread's says which", async () => {
  await render(<View state={chat('reviewAndFix')} />);
  await expect.element(page.getByText('Add the feature')).toBeVisible();
  const lines = () => replies().map((r) => r.querySelector('.name-line')?.textContent ?? '');
  await expect.poll(() => lines().some((l) => l.startsWith('Vale'))).toBe(true);
  // Vale's review is its own task, in the work's thread: it says which.
  expect(lines().find((l) => l.startsWith('Vale'))).toContain('on Review #5');
});

test("a reply on the thread's own task doesn't name it again", async () => {
  await render(<View state={chat('askDone')} />);
  await expect.element(page.getByText('Add the tags table')).toBeVisible();
  await (foldOf(mainWith('Add the tags table')) as HTMLButtonElement).click();
  const ann = () => replies().find((r) => r.querySelector('.name-line-name')?.textContent === 'Ann');
  await expect.poll(() => !!ann()).toBe(true);
  expect(ann()?.querySelector('.name-line')?.textContent).not.toContain('on Add the tags table');
  expect(ann()?.querySelector('.name-line-task')).toBeNull();
});

test("the result is at the bottom quoting its ask; the quote leads to the ask, and the ask's Done to it", async () => {
  await render(<View state={chat('askDone')} />);
  const all = document.querySelectorAll('.thread-messages > .said:not([data-reply])');
  const result = all[all.length - 1] as HTMLElement;
  await expect.element(page.getByRole('button', { name: 'Add tags to notes' })).toBeVisible();
  expect(result.querySelector('.bubble-quote')?.textContent).toBe('Add tags to notes');
  expect(result.querySelector('.name-line')?.textContent).toContain('Result');
  const ask = mainWith('Add tags to notes');
  await page.getByRole('button', { name: 'Add tags to notes' }).click();
  await expect.poll(() => ask.hasAttribute('data-search-hit')).toBe(true);
  const done = ask.querySelector('.main-state-link') as HTMLButtonElement;
  expect(done.textContent).toBe('Done');
  done.focus();
  await userEvent.keyboard('{Enter}');
  await expect.poll(() => result.hasAttribute('data-search-hit')).toBe(true);
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
  await expect.poll(() => mainWith('Add tags to notes').textContent).toContain('Done');
  expect(fold().getAttribute('aria-expanded')).toBe('true');
});

test('a chat of one agent is one timeline: no state word and no fold line', async () => {
  const state = chat('askDone');
  await render(<View state={{ ...state, members: state.members.slice(0, 1) }} />);
  await expect.element(page.getByText('Add tags to notes', { exact: true })).toBeVisible();
  expect(document.querySelectorAll('.main-state, .fold, [data-reply]')).toHaveLength(0);
});

test('what came before the chat became a team stays one timeline; threads begin after', async () => {
  const state = chat('askDone');
  // The second agent joined after the ask and the lead's first turn.
  const since = state.messages.find((m) => m.body === 'Hi!' && m.turn === state.messages[0].turn)
    ?.time as string;
  await render(<View state={{ ...state, teamSince: new Date(Date.parse(since) + 1).toISOString() }} />);
  await expect.element(page.getByText('Add the tags table')).toBeVisible();
  const ask = mainWith('Add tags to notes');
  expect(ask.querySelector('.main-state')).toBeNull();
  expect(ask.nextElementSibling?.classList.contains('fold')).toBe(false);
  expect(mains().map(([, s]) => s)).toContain('Done');
});

test('an old chat, tied by task and answers alone, reads as main messages with threads', async () => {
  await render(<View state={chat('oldChat')} />);
  await expect.element(page.getByText('Sort the gravel bars', { exact: true })).toBeVisible();
  // The person's message, the task handed out, and the turn nothing ties, its own main message. Its times are a few
  // microseconds apart, closer than a message's time says, so only which ones are main is checked.
  expect(
    mains()
      .map(([who]) => who)
      .sort(),
  ).toEqual(['Gathr', 'Mira', 'You']);
  // The task is done: its thread starts folded.
  await (foldOf(mainWith('@Rowan')) as HTMLButtonElement).click();
  await expect.element(page.getByText('Sorted, herons first')).toBeVisible();
  const rowan = replies().find((r) => r.textContent?.includes('Sorted, herons first'));
  const fold = mainWith('@Rowan').nextElementSibling as Element;
  expect(fold.compareDocumentPosition(rowan as Element) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
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
