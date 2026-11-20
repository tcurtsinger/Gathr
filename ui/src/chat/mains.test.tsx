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

/** The main messages drawn, top to bottom. */
const mainEls = () => [
  ...document.querySelectorAll<HTMLElement>('.thread-messages > .said:not([data-reply])'),
];

test("the lead's answer is a main message right under yours, its plan chip in its bubble with its words", async () => {
  await render(<View state={chat('askAtWork')} />);
  await expect.element(page.getByText('Add the tags table')).toBeVisible();
  // Your ask, the lead's answer, and the two tasks it handed out: no main message is a chip alone (0078).
  expect(mains()).toEqual(['You', 'Lead', 'Lead', 'Lead']);
  const [ask, answer] = mainEls();
  expect(ask.textContent).toContain('Add tags to notes');
  expect(foldOf(ask)).toBeNull();
  const bubble = answer.querySelector('.bubble') as HTMLElement;
  const chip = bubble.querySelector('.plan-chip') as HTMLElement;
  expect(document.querySelectorAll('.plan-chip')).toHaveLength(1);
  // The chip ends the message, after the words the lead wrote, and says where the version stands (the owner's calls
  // of 2026-11-19).
  const words = [...bubble.querySelectorAll('p')].find((p) => p.textContent === 'Hi!') as HTMLElement;
  expect(words.compareDocumentPosition(chip) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(chip.textContent).toMatch(/^Plan v\d+:Approved$/);
});

test("the plan's check reads in the thread of the lead's bubble, by keyboard too; the person's Approve is no line there", async () => {
  await render(<View state={chat('askAtWork')} />);
  await expect.element(page.getByText('Add the tags table')).toBeVisible();
  const answer = mainEls()[1];
  // The plan was decided when the chat opened: its thread starts folded.
  const fold = foldOf(answer) as HTMLButtonElement;
  expect(fold.getAttribute('aria-expanded')).toBe('false');
  fold.focus();
  await userEvent.keyboard('{Enter}');
  await expect
    .poll(() => replies().map((r) => r.querySelector('.name-line-name')?.textContent))
    .toContain('Rev');
  const rev = replies().find((r) => r.querySelector('.name-line-name')?.textContent === 'Rev') as Element;
  expect(answer.compareDocumentPosition(rev) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  // Before the next main message: in this thread.
  const next = mainEls()[2];
  expect(rev.compareDocumentPosition(next) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  // The version's chip says Approved; Gathr says no line of its own for it.
  expect(replies().filter((r) => r.textContent?.includes('Approved'))).toEqual([]);
});

/** The lead's turn `id`, opened by Gathr's `opening` words if any, and answering with `words`, at minute `at`. */
/** `s` seconds after the chat's last message: what is added to a recorded chat comes after it. */
function later(state: ChatState, s: number) {
  return new Date(Date.parse(state.messages.at(-1)?.time ?? '') + s * 1000).toISOString();
}

function leadTurn(
  state: ChatState,
  id: string,
  opening: string | null,
  words: string,
  at: number,
): ChatState {
  const lead = state.members[0].agent;
  const turn = { id, agent: lead, status: 'done', why: null, ref: id, position: 90 + at, topic: `b:${id}` };
  const said = (n: number, role: Message['role'], body: string): Message => ({
    id: `${id}-${n}`,
    turn: id,
    time: later(state, at * 60 + n),
    role,
    kind: 'text',
    body,
  });
  return {
    ...state,
    turns: { ...state.turns, [id]: turn as Turn },
    messages: [
      ...state.messages,
      ...(opening ? [said(1, 'gathr', opening)] : []),
      said(2, 'assistant', words),
    ],
  };
}

test("a wait's result opens the lead's bubble as its quote, with no message of Gathr's for it", async () => {
  const state = leadTurn(
    chat('askDone'),
    'waited',
    'Made-up: your wait ended, the build is green',
    'Made-up go on',
    1,
  );
  await render(<View state={state} />);
  await expect.element(page.getByText('Made-up go on')).toBeVisible();
  const bubble = mainWith('Made-up go on');
  // The lead's own message (after its result, by the same speaker, so not named again).
  expect(bubble.getAttribute('data-agent')).toBe(state.members[0].agent);
  expect(bubble.querySelector('.bubble-quote')?.textContent).toBe(
    'Made-up: your wait ended, the build is green',
  );
  const gathrs = [...document.querySelectorAll('.said[data-gathr]')];
  expect(gathrs.some((g) => g.textContent?.includes('your wait ended'))).toBe(false);
});

test("your message into the lead's running turn: what it says after is a main message of its own", async () => {
  const base = chat('askDone');
  const lead = base.members[0].agent;
  const busy = {
    id: 'busy',
    agent: lead,
    status: 'running',
    why: null,
    ref: 'busy',
    position: 99,
    topic: 'b:busy',
  };
  const said = (id: string, role: Message['role'], body: string, s: number): Message => ({
    id,
    turn: 'busy',
    time: later(base, s),
    role,
    kind: 'text',
    body,
  });
  const state = {
    ...base,
    turns: { ...base.turns, busy: busy as Turn },
    messages: [
      ...base.messages,
      said('b1', 'user', 'Made-up first ask', 1),
      said('b2', 'assistant', 'Made-up start', 2),
      said('b3', 'user', 'Made-up second word', 3),
      said('b4', 'assistant', 'Made-up after it', 4),
    ],
  };
  await render(<View state={state} />);
  await expect.element(page.getByText('Made-up after it')).toBeVisible();
  const last = mainEls().slice(-4);
  // The words of each bubble; the running turn's working line stays at its foot (0082).
  const words = (m: Element) => m.querySelector('.bubble > :not(.work)')?.textContent;
  expect(last.map(words)).toEqual([
    'Made-up first ask',
    'Made-up start',
    'Made-up second word',
    'Made-up after it',
  ]);
  expect(last.map((m) => m.querySelector('.name-line-name')?.textContent)).toEqual([
    'You',
    'Lead',
    'You',
    'Lead',
  ]);
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
  // The person's ask, the lead's answer to it (0078), and the task it handed out.
  expect(mains()).toEqual(['You', 'Lead', 'Lead']);
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
  const main = mainWith('Plan v1:');
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
    time: later(state, 1),
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
  // Ann is at work on the task: its thread starts open.
  const fold = () => foldOf(mainWith('Add the tags table')) as HTMLButtonElement;
  await expect.poll(() => fold()?.getAttribute('aria-expanded')).toBe('true');
  // The same chat once all is in: the task is done, its thread handled.
  await screen.rerender(<View state={chat('askDone')} />);
  // Its result, said once all is in, is drawn.
  await expect.poll(() => document.querySelector('.bubble-quote')?.textContent).toBe('Add tags to notes');
  expect(fold().getAttribute('aria-expanded')).toBe('true');
});

test('a chat of one reads as one timeline by the same rule; a reply to an older answer makes its thread', async () => {
  await render(<View state={chat('chatOfOne')} />);
  await expect.element(page.getByText('And the map legend?')).toBeVisible();
  // Each answer a main message under the message it answers; one fold line, on the answer replied to.
  expect(mains()).toEqual(['You', 'Mira', 'You', 'Mira']);
  const [, first, , second] = mainEls();
  expect(document.querySelectorAll('.fold-line')).toHaveLength(1);
  expect(foldOf(first)?.textContent).toMatch(/2 replies|Collapse replies/);
  expect(foldOf(second)).toBeNull();
  if (foldOf(first)?.getAttribute('aria-expanded') === 'false')
    await (foldOf(first) as HTMLButtonElement).click();
  await expect.element(page.getByText('Does it pass the old mill?')).toBeVisible();
  const reply = replies().find((r) => r.textContent?.includes('Does it pass the old mill?')) as Element;
  expect(first.compareDocumentPosition(reply) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(reply.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});

test('a main message right after the same speaker’s main message is not named again', async () => {
  const base = chat('askDone');
  const nudge: Message = {
    id: 'nudge',
    turn: 'one',
    time: later(base, 59),
    role: 'user',
    kind: 'text',
    body: 'Made-up nudge',
  };
  const asked = leadTurn({ ...base, messages: [...base.messages, nudge] }, 'one', null, 'Made-up one', 1);
  const twice = leadTurn(asked, 'two', 'Made-up: a wait ended', 'Made-up two', 2);
  await render(<View state={twice} />);
  await expect.element(page.getByText('Made-up two')).toBeVisible();
  expect(mainWith('Made-up one').querySelector('.name-line-name')?.textContent).toBe('Lead');
  expect(mainWith('Made-up two').querySelector('.name-line-name')).toBeNull();
});

// The owner, 2026-11-19: the lead's second plan had no name while the replies under its first were folded, and had
// one once they were open.
test('a main message after one with a thread is named, whether that thread is folded or open', async () => {
  const base = chat('askDone');
  const one = leadTurn(base, 'one', null, 'Made-up one', 1);
  const two = leadTurn(one, 'two', 'Made-up: a wait ended', 'Made-up two', 2);
  const member = two.members[1].agent;
  const check = {
    id: 'chk',
    agent: member,
    status: 'done',
    why: null,
    ref: 'chk',
    position: 91.5,
    topic: 'b:one',
  };
  const said: Message = {
    id: 'chk-1',
    turn: 'chk',
    time: later(base, 90),
    role: 'assistant',
    kind: 'text',
    body: 'Made-up check',
  };
  const state = { ...two, turns: { ...two.turns, chk: check as Turn }, messages: [...two.messages, said] };
  await render(<View state={state} />);
  await expect.element(page.getByText('Made-up two')).toBeVisible();
  const fold = foldOf(mainWith('Made-up one')) as HTMLButtonElement;
  const named = () => mainWith('Made-up two').querySelector('.name-line-name')?.textContent;
  for (const open of [fold.getAttribute('aria-expanded'), undefined]) {
    if (open === undefined) await fold.click();
    expect(named(), `open: ${fold.getAttribute('aria-expanded')}`).toBe('Lead');
  }
});

test('an old chat, tied by task and answers alone, reads as main messages with threads', async () => {
  await render(<View state={chat('oldChat')} />);
  await expect.element(page.getByText('Sort the gravel bars', { exact: true })).toBeVisible();
  // The person's message, the lead's answer to it, the task handed out, and the lead's two later turns with Gathr's
  // words and none of their own (0078): each its own main message, Gathr's words as its line, as they have no bubble
  // to quote them in. Its times are a few microseconds apart, closer than a message's time says, so only which ones
  // are main is checked.
  expect(mains().sort()).toEqual(['Gathr', 'Gathr', 'Mira', 'Mira', 'You']);
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
