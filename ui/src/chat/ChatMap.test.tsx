// The conversation map beside the timeline (0039): when it shows, which tick is current as the chat
// scrolls, and a tick opening its message, drawn or not.
import { useRef } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message, Turn } from '../api';
import { toThread } from './messages';
import { type ChatState, empty } from './state';
import { type Actions, Thread } from './Thread';

// Wednesday 11 November 2026, in the machine's own timezone.
beforeEach(async () => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date(2026, 10, 11, 18, 0));
  await page.viewport(1280, 1100);
});
afterEach(() => {
  vi.useRealTimers();
});

const none: Actions = {
  openTask: () => {},
  openAgent: () => {},
  diagnostics: () => {},
  reference: () => {},
  signIn: () => {},
  retry: () => {},
  resend: () => {},
  carryOn: () => {},
  approve: () => {},
};

/** A day of a one-agent chat: asks on the hour from 09:00, each answered at half past. */
function day(date: number, asks: number, answer = 'Made-up answer'): { turns: Turn[]; messages: Message[] } {
  const turns: Turn[] = [];
  const messages: Message[] = [];
  for (let n = 0; n < asks; n++) {
    const id = `${date}-${n}`;
    const time = (minute: number) => new Date(2026, 10, date, 9 + (n % 12), minute, n).toISOString();
    turns.push({ id, agent: 'mira', status: 'done', why: null, ref: id });
    messages.push(
      { id: `u${id}`, turn: id, time: time(0), role: 'user', kind: 'text', body: `Made-up ask ${n}` },
      { id: `a${id}`, turn: id, time: time(30), role: 'assistant', kind: 'text', body: answer },
    );
  }
  return { turns, messages };
}
function chat(...days: ReturnType<typeof day>[]): ChatState {
  const turns = days.flatMap((d) => d.turns);
  return {
    ...empty,
    members: [{ agent: 'mira', name: 'Mira', role: 'supervisor' }],
    messages: days.flatMap((d) => d.messages),
    turns: Object.fromEntries(turns.map((t, position) => [t.id, { ...t, position }])),
  };
}
// Each answer is taller than the chat's view.
const tall = Array.from({ length: 40 }, (_, n) => `Made-up line ${n} of the answer.`).join('\n\n');

function View({ state, height = 600 }: { state: ChatState; height?: number }) {
  const cache = useRef(new Map());
  return (
    <div style={{ height, display: 'flex' }}>
      <Thread
        messages={toThread(state, [], [], cache.current).items}
        running={false}
        actions={none}
        empty={false}
      >
        <p style={{ height: 40, margin: 0 }}>Composer</p>
      </Thread>
    </div>
  );
}

const ticks = () => [...document.querySelectorAll<HTMLElement>('[data-slot="conversation-map-tick"]')];
const names = (all = ticks()) => all.map((t) => t.getAttribute('aria-label'));
const tick = (name: string) => ticks().find((t) => t.getAttribute('aria-label') === name) as HTMLElement;
const viewport = () => document.querySelector('.thread-viewport') as HTMLElement;
/** How far below the chat's top edge a message's block starts. */
const below = (id: string) => {
  const block = document.querySelector(`[data-message-id="${id}"]`);
  return block ? block.getBoundingClientRect().top - viewport().getBoundingClientRect().top : undefined;
};

test('the map shows from three ticks up, long for the person and short for the lead, in a pane wide enough', async () => {
  const screen = await render(<View state={chat(day(11, 1))} />);
  await expect.element(screen.getByText('Made-up ask 0')).toBeVisible();
  expect(ticks()).toHaveLength(0);

  await screen.rerender(<View state={chat(day(11, 2))} />);
  await expect.poll(() => names()).toEqual(['You · 09:00', 'Mira · 09:30', 'You · 10:00', 'Mira · 10:30']);
  const width = (name: string) => getComputedStyle(tick(name).firstElementChild as Element).width;
  expect([width('You · 09:00'), width('Mira · 09:30')]).toEqual(['12px', '6px']);
  // Clear of the column's text.
  const column = document.querySelector('.thread-column') as HTMLElement;
  expect(tick('You · 09:00').getBoundingClientRect().right).toBeLessThan(column.getBoundingClientRect().left);

  await page.viewport(700, 1100);
  await expect.poll(() => ticks().length).toBe(0);
});

test('the current tick is the latest message at or above the middle of the chat, and ticks on screen are in view', async () => {
  await render(<View state={chat(day(11, 6, tall))} />);
  const current = () => names(ticks().filter((t) => t.getAttribute('aria-current')));
  const inView = () => names(ticks().filter((t) => t.hasAttribute('data-in-view')));
  // It opens at its latest message.
  await expect.poll(current).toEqual(['Mira · 14:30']);
  expect(inView()).toEqual(['Mira · 14:30']);

  viewport().scrollTop = 0;
  await expect.poll(current).toEqual(['Mira · 09:30']);
  expect(inView()).toEqual(['You · 09:00', 'Mira · 09:30']);

  // The third ask, just above the middle of what shows over the composer: the answer before it fills the top.
  const footer = document.querySelector('.thread-footer') as HTMLElement;
  const middle = (footer.getBoundingClientRect().top - viewport().getBoundingClientRect().top) / 2;
  viewport().scrollTop += (below('u11-2') as number) - (middle - 30);
  await expect.poll(current).toEqual(['You · 11:00']);
  expect(inView()).toEqual(['Mira · 10:30', 'You · 11:00', 'Mira · 11:30']);
});

test('a tick opens its message at the top of the chat, by pointer or keys, and focus stays on the map', async () => {
  await render(<View state={chat(day(11, 6, tall))} />);
  await expect.poll(() => ticks().length).toBe(12);
  await userEvent.click(tick('You · 11:00'));
  await expect.poll(() => Math.round(below('u11-2') ?? -1)).toBe(32);
  expect(document.activeElement).toBe(tick('You · 11:00'));

  await userEvent.keyboard('{ArrowDown}');
  expect(document.activeElement).toBe(tick('Mira · 11:30'));
  await userEvent.keyboard('{Enter}');
  await expect.poll(() => Math.round(below('a-11-2') ?? -1)).toBe(32);
  expect(document.activeElement).toBe(tick('Mira · 11:30'));

  await userEvent.keyboard('{End}');
  expect(document.activeElement).toBe(tick('Mira · 14:30'));
  await userEvent.keyboard('{ArrowUp}{ArrowUp}');
  expect(document.activeElement).toBe(tick('Mira · 13:30'));
  await userEvent.keyboard('{Home}');
  expect(document.activeElement).toBe(tick('You · 09:00'));
});

test('a shorter pane folds the oldest days first, and its ticks keep their distance', async () => {
  // Sunday to today: 80 ticks.
  const state = chat(day(8, 20), day(9, 10), day(10, 5), day(11, 5));
  const screen = await render(<View state={state} height={1000} />);
  await expect.poll(() => ticks().length).toBe(80);

  await screen.rerender(<View state={state} height={420} />);
  await expect.poll(() => ticks().length).toBe(41);
  expect(names()[0]).toBe('Sunday · 20 messages from you');

  await screen.rerender(<View state={state} height={260} />);
  await expect.poll(() => ticks().length).toBe(22);
  expect(names().slice(0, 3)).toEqual([
    'Sunday · 20 messages from you',
    'Monday · 10 messages from you',
    'You · 09:00',
  ]);
  // A day's tick is the thick one.
  const monday = () => tick('Monday · 10 messages from you').firstElementChild as Element;
  await expect.poll(() => getComputedStyle(monday()).height).toBe('4px');
  // Ticks keep 6px each.
  const tops = ticks().map((t) => t.getBoundingClientRect().top);
  expect(Math.min(...tops.slice(1).map((top, i) => top - tops[i]))).toBeGreaterThanOrEqual(6);
});

test("a day's tick opens the day's first message, though it was not drawn", async () => {
  // Monday's first message is 220 blocks from the end, and the chat draws its latest 100.
  await render(<View state={chat(day(8, 3), day(9, 100), day(10, 5), day(11, 5))} />);
  await expect
    .poll(() => names().slice(0, 2))
    .toEqual(['Sunday · 3 messages from you', 'Monday · 100 messages from you']);
  expect(below('u9-0')).toBeUndefined();
  await userEvent.click(tick('Monday · 100 messages from you'));
  await expect.poll(() => Math.round(below('u9-0') ?? -1)).toBe(32);
  await expect.poll(() => tick('Monday · 100 messages from you').getAttribute('aria-current')).toBe('true');
});
