// Agents' thumbs-ups on the person's own messages (0057, the threads build's PR 7): the count under the bubble with who
// agreed, as on an agent's message, on the fold line's row; as a main message, as a reply in a thread, in a chat of one
// agent, and live. The person can't like their own: no Like on it, and its count isn't a toggle. Pointer and keyboard.
import { expect, test, vi } from 'vitest';
import { page, server, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message, Turn } from '../api';
import { chat, View } from './recorded';
import { apply, type ChatState } from './state';

const WORDS = 'Add tags to notes';

/** The recorded chat with the person's ask, "Add tags to notes", and its lead, Ann and Rev. */
function askDone() {
  const state = chat('askDone');
  const ask = state.messages.find((m) => m.role === 'user')?.id as string;
  const [lead, ann] = state.members.map((m) => m.agent);
  return { state, ask, lead, ann };
}

const given = (message: string, agent: string, minute: number) => ({
  message,
  agent,
  time: `2026-11-18T23:${String(minute).padStart(2, '0')}:00Z`,
});

/** The drawn message holding these words: a main message or a reply. */
function saidWith(words: string): HTMLElement {
  const found = [...document.querySelectorAll<HTMLElement>('.thread-messages > .said')].find((el) =>
    el.querySelector('.bubble')?.textContent?.includes(words),
  );
  if (!found) throw new Error(`no message says ${words}`);
  return found;
}
const countIn = (said: HTMLElement, names: string) =>
  page.elementLocator(said).getByRole('button', { name: `Thumbs-up from ${names}` });
const shown = (el: Element) => getComputedStyle(el).display !== 'none';

test("an agent's thumbs-up on the person's message: the count beside its fold line, who by pointer and keyboard", async () => {
  const { state, ask, lead, ann } = askDone();
  const like = vi.fn();
  await render(
    <View state={{ ...state, reactions: [given(ask, ann, 1), given(ask, lead, 2)] }} actions={{ like }} />,
  );
  const main = await vi.waitFor(() => saidWith(WORDS));
  const count = countIn(main, 'Ann and Lead');
  await expect.element(count).toBeVisible();
  const row = main.querySelector('.said-foot') as HTMLElement;
  expect(row.contains(count.element())).toBe(true);
  expect([...count.element().children].slice(0, 2).map((c) => c.textContent)).toEqual(['👍', '2']);
  // On the replies' row, after the fold line: their middles level.
  const line = (row.querySelector('.fold-line') as HTMLElement).getBoundingClientRect();
  const mark = count.element().getBoundingClientRect();
  expect(mark.left).toBeGreaterThan(line.right);
  expect(Math.abs((line.top + line.bottom) / 2 - (mark.top + mark.bottom) / 2)).toBeLessThan(2);

  const who = row.querySelector('.react-who') as HTMLElement;
  expect(shown(who)).toBe(false);
  await userEvent.hover(count.element());
  await expect.poll(() => shown(who)).toBe(true);
  expect(who.textContent).toBe('AnnLead');
  await userEvent.unhover(count.element());
  await expect.poll(() => shown(who)).toBe(false);

  // By keyboard, as a person reaches it. Safari's Tab skips buttons unless the person turns that on; Option+Tab reaches them.
  const tab = server.browser === 'webkit' ? '{Alt>}{Tab}{/Alt}' : '{Tab}';
  for (let i = 0; i < 60 && document.activeElement !== count.element(); i++) await userEvent.keyboard(tab);
  expect(document.activeElement).toBe(count.element());
  await expect.poll(() => shown(who)).toBe(true);
});

test('the person can’t like their own message: no Like on it, and its count is no toggle', async () => {
  const { state, ask, ann } = askDone();
  const like = vi.fn();
  await render(<View state={{ ...state, reactions: [given(ask, ann, 1)] }} actions={{ like }} />);
  const main = await vi.waitFor(() => saidWith(WORDS));
  const count = countIn(main, 'Ann');
  await expect.element(count).toBeVisible();
  // An agent's message in the same chat offers it.
  expect(document.querySelector('button[aria-label="Like"]')).not.toBeNull();
  expect(main.querySelector('button[aria-label="Like"]')).toBeNull();
  expect(count.element().getAttribute('aria-pressed')).toBeNull();
  await count.click();
  (count.element() as HTMLElement).focus();
  await userEvent.keyboard('{Enter}');
  await expect.poll(() => countIn(saidWith(WORDS), 'Ann').element().getAttribute('aria-pressed')).toBeNull();
  expect(like).not.toHaveBeenCalled();
});

test("an agent's thumbs-up on the person's reply in a thread shows under the reply", async () => {
  const { state, ask, ann } = askDone();
  // The lead's last words in the ask's thread: the reply answers them, so it sits there too.
  const answer = state.messages.findLast(
    (m) => m.role === 'assistant' && m.kind === 'text' && state.turns[m.turn ?? '']?.topic === `m:${ask}`,
  ) as Message;
  const turn: Turn = {
    id: 'made-up-reply-turn',
    agent: state.members[0].agent,
    status: 'done',
    why: null,
    ref: 'made0up2',
    position: 99,
    topic: `m:${ask}`,
  };
  const reply: Message = {
    id: 'made-up-reply',
    turn: turn.id,
    time: '2026-11-18T23:00:00Z',
    role: 'user',
    kind: 'text',
    body: 'Made-up reply of the person',
    reply_to: answer.id,
  };
  const more: ChatState = {
    ...state,
    turns: { ...state.turns, [turn.id]: turn },
    messages: [...state.messages, reply],
    reactions: [given(reply.id, ann, 1)],
  };
  await render(<View state={more} />);
  const main = await vi.waitFor(() => saidWith(WORDS));
  const fold = main.querySelector('.fold-line') as HTMLButtonElement;
  if (fold.getAttribute('aria-expanded') === 'false') await userEvent.click(fold);
  const said = await vi.waitFor(() => saidWith('Made-up reply of the person'));
  expect(said.hasAttribute('data-reply')).toBe(true);
  await expect.element(countIn(said, 'Ann')).toBeVisible();
  // The main message has none of its own.
  expect(main.querySelector('.react')).toBeNull();
});

test("in a chat of one agent, drawn flat, the agent's thumbs-up shows under the person's message", async () => {
  const { state, ask, lead } = askDone();
  const solo = { ...state, members: state.members.slice(0, 1), reactions: [given(ask, lead, 1)] };
  await render(<View state={solo} />);
  const said = await vi.waitFor(() => saidWith(WORDS));
  await expect.element(countIn(said, 'Lead')).toBeVisible();
  expect(said.querySelector('.fold-line')).toBeNull();
});

test("an agent's thumbs-up on the person's message arrives live", async () => {
  const { state, ask, ann } = askDone();
  const screen = await render(<View state={state} />);
  const main = await vi.waitFor(() => saidWith(WORDS));
  expect(main.querySelector('.react')).toBeNull();
  const live = apply(state, { type: 'reaction', chat: 'c', reaction: given(ask, ann, 3) });
  await screen.rerender(<View state={live} />);
  await expect.element(countIn(saidWith(WORDS), 'Ann')).toBeVisible();
});
