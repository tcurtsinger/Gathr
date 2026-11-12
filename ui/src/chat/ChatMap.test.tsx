// The conversation map beside the timeline (0039): when it shows, which tick is current as the chat
// scrolls, and a tick opening its message, drawn or not.
import { useRef } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message, Turn } from '../api';
import { toThread } from './messages';
import { type ChatState, empty } from './state';
import { type Actions, type Target, Thread } from './Thread';

// Wednesday 11 November 2026, in the machine's own timezone.
beforeEach(async () => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date(2026, 10, 11, 18, 0));
  await page.viewport(1280, 1100);
});
afterEach(() => {
  vi.useRealTimers();
});

// Times and days as the reader's locale writes them: the map formats with it too.
const hhmm = (hour: number, minute: number) =>
  new Date(2026, 10, 11, hour, minute).toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });
const weekday = (date: number) => new Date(2026, 10, date).toLocaleDateString([], { weekday: 'long' });

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

interface ViewProps {
  state: ChatState;
  height?: number;
  target?: Target;
  onTarget?: () => void;
}
function View({ state, height = 600, target, onTarget }: ViewProps) {
  const cache = useRef(new Map());
  return (
    <div style={{ height, display: 'flex' }}>
      <Thread
        messages={toThread(state, [], [], cache.current).items}
        running={false}
        actions={none}
        empty={false}
        target={target}
        onTarget={onTarget}
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
// At the top of the view: 32px under its edge. WebKit scrolls by whole pixels and blocks sit on
// fractions of one, so a message can land up to a pixel off: WebKit drops the fraction rather than
// rounding it (#154's 1.5 line height moved blocks to fractions above a half).
/** How far a message is from the top's 32px, in pixels; a failure prints it (G177). */
const offTop = (id: string) => Math.abs((below(id) ?? Number.NaN) - 32);

test('the map shows from three ticks up, long for the person and short for the lead, in a pane wide enough', async () => {
  const screen = await render(<View state={chat(day(11, 1))} />);
  await expect.element(screen.getByText('Made-up ask 0')).toBeVisible();
  expect(ticks()).toHaveLength(0);

  await screen.rerender(<View state={chat(day(11, 2))} />);
  await expect
    .poll(() => names())
    .toEqual([
      `You · ${hhmm(9, 0)}`,
      `Mira · ${hhmm(9, 30)}`,
      `You · ${hhmm(10, 0)}`,
      `Mira · ${hhmm(10, 30)}`,
    ]);
  const width = (name: string) => getComputedStyle(tick(name).firstElementChild as Element).width;
  expect([width(`You · ${hhmm(9, 0)}`), width(`Mira · ${hhmm(9, 30)}`)]).toEqual(['12px', '6px']);
  // Clear of the column's text.
  const column = document.querySelector('.thread-column') as HTMLElement;
  expect(tick(`You · ${hhmm(9, 0)}`).getBoundingClientRect().right).toBeLessThan(
    column.getBoundingClientRect().left,
  );

  // A screen reader hears the preview too.
  expect(tick(`Mira · ${hhmm(9, 30)}`).getAttribute('aria-description')).toBe('Made-up answer');
  // Four ticks leave most of the rail empty: there the pointer, and so the wheel, reaches the chat.
  const rail = (document.querySelector('.chat-map') as HTMLElement).getBoundingClientRect();
  const under = document.elementFromPoint(rail.left + 6, rail.top + 10);
  expect(under?.closest('.chat-map')).toBeNull();
  expect(under?.closest('.thread-viewport')).not.toBeNull();

  await page.viewport(700, 1100);
  await expect.poll(() => ticks().length).toBe(0);
});

/**
 * The map follows a scroll at the page's next frames. On WebKit on CI it once hadn't within the poll's default
 * second (main at 9fe0852; 120 runs since didn't), cause unproven: a slow page or a missed scroll. It gets five,
 * and a failure prints what it measured since the scroll, so few frames read apart from a missed scroll (G177).
 */
const FOLLOW = { timeout: 5000 };
function following(id: string, middle: number) {
  const el = viewport();
  const since = { frames: 0, scrolls: 0 };
  const count = () => {
    since.frames += 1;
    if (el.isConnected) requestAnimationFrame(count);
  };
  requestAnimationFrame(count);
  el.addEventListener('scroll', () => since.scrolls++);
  return () => ({
    current: names(ticks().filter((t) => t.getAttribute('aria-current'))),
    scrollTop: el.scrollTop,
    fromMiddle: (below(id) ?? Number.NaN) - middle,
    ...since,
  });
}

/** The current tick `current`, with what was measured beside it, so a failure prints those values. */
const measured = (current: string[]) => ({
  current,
  scrollTop: expect.any(Number),
  fromMiddle: expect.any(Number),
  frames: expect.any(Number),
  scrolls: expect.any(Number),
});

test('the current tick is the latest message at or above the middle of the chat, and ticks on screen are in view', async () => {
  await render(<View state={chat(day(11, 6, tall))} />);
  const current = () => names(ticks().filter((t) => t.getAttribute('aria-current')));
  const inView = () => names(ticks().filter((t) => t.hasAttribute('data-in-view')));
  // It opens at its latest message.
  await expect.poll(current).toEqual([`Mira · ${hhmm(14, 30)}`]);
  expect(inView()).toEqual([`Mira · ${hhmm(14, 30)}`]);

  const footer = document.querySelector('.thread-footer') as HTMLElement;
  const middle = (footer.getBoundingClientRect().top - viewport().getBoundingClientRect().top) / 2;
  viewport().scrollTop = 0;
  await expect.poll(following('u11-2', middle), FOLLOW).toEqual(measured([`Mira · ${hhmm(9, 30)}`]));
  expect(inView()).toEqual([`You · ${hhmm(9, 0)}`, `Mira · ${hhmm(9, 30)}`]);

  // The third ask, just above the middle of what shows over the composer: the answer before it fills the top.
  viewport().scrollTop += (below('u11-2') as number) - (middle - 30);
  await expect.poll(following('u11-2', middle), FOLLOW).toEqual(measured([`You · ${hhmm(11, 0)}`]));
  expect(inView()).toEqual([`Mira · ${hhmm(10, 30)}`, `You · ${hhmm(11, 0)}`, `Mira · ${hhmm(11, 30)}`]);
});

test('a tick opens its message at the top of the chat, by pointer or keys, and focus stays on the map', async () => {
  await render(<View state={chat(day(11, 6, tall))} />);
  await expect.poll(() => ticks().length).toBe(12);
  await userEvent.click(tick(`You · ${hhmm(11, 0)}`));
  await expect.poll(() => offTop('u11-2')).toBeLessThan(1);
  expect(document.activeElement).toBe(tick(`You · ${hhmm(11, 0)}`));

  await userEvent.keyboard('{ArrowDown}');
  expect(document.activeElement).toBe(tick(`Mira · ${hhmm(11, 30)}`));
  await userEvent.keyboard('{Enter}');
  await expect.poll(() => offTop('a-11-2')).toBeLessThan(1);
  expect(document.activeElement).toBe(tick(`Mira · ${hhmm(11, 30)}`));

  await userEvent.keyboard('{End}');
  expect(document.activeElement).toBe(tick(`Mira · ${hhmm(14, 30)}`));
  await userEvent.keyboard('{ArrowUp}{ArrowUp}');
  expect(document.activeElement).toBe(tick(`Mira · ${hhmm(13, 30)}`));
  await userEvent.keyboard('{Home}');
  expect(document.activeElement).toBe(tick(`You · ${hhmm(9, 0)}`));
});

test('a shorter pane folds the oldest days first, and its ticks keep their distance', async () => {
  // Sunday to today: 80 ticks.
  const state = chat(day(8, 20), day(9, 10), day(10, 5), day(11, 5));
  // The rail is half the pane, 83 ticks' worth here, with as much space above it as below.
  const screen = await render(<View state={state} height={1000} />);
  await expect.poll(() => ticks().length).toBe(80);
  const gaps = () => {
    const pane = (document.querySelector('.thread') as HTMLElement).getBoundingClientRect();
    const rail = (document.querySelector('.chat-map') as HTMLElement).getBoundingClientRect();
    return [rail.top - pane.top, rail.height, pane.bottom - rail.bottom];
  };
  expect(gaps()).toEqual([250, 500, 250]);

  // Never longer than 616px, however tall the pane.
  await screen.rerender(<View state={state} height={1400} />);
  await expect.poll(gaps).toEqual([392, 616, 392]);

  await screen.rerender(<View state={state} height={600} />);
  await expect.poll(() => ticks().length).toBe(41);
  expect(names()[0]).toBe(`${weekday(8)} · 20 messages from you`);

  await screen.rerender(<View state={state} height={300} />);
  await expect.poll(() => ticks().length).toBe(22);
  expect(names().slice(0, 3)).toEqual([
    `${weekday(8)} · 20 messages from you`,
    `${weekday(9)} · 10 messages from you`,
    `You · ${hhmm(9, 0)}`,
  ]);
  // A day's tick is the thick one.
  const monday = () => tick(`${weekday(9)} · 10 messages from you`).firstElementChild as Element;
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
    .toEqual([`${weekday(8)} · 3 messages from you`, `${weekday(9)} · 100 messages from you`]);
  expect(below('u9-0')).toBeUndefined();
  await userEvent.click(tick(`${weekday(9)} · 100 messages from you`));
  await expect.poll(() => offTop('u9-0')).toBeLessThan(1);
  await expect
    .poll(() => tick(`${weekday(9)} · 100 messages from you`).getAttribute('aria-current'))
    .toBe('true');
});

test('at the end of the chat the latest message on screen is current, though it sits below the middle', async () => {
  await render(<View state={chat(day(11, 6))} />);
  const current = () => names(ticks().filter((t) => t.getAttribute('aria-current')));
  await expect.poll(current).toEqual([`Mira · ${hhmm(14, 30)}`]);
  const footer = document.querySelector('.thread-footer') as HTMLElement;
  const middle = (footer.getBoundingClientRect().top - viewport().getBoundingClientRect().top) / 2;
  expect(below('a-11-5')).toBeGreaterThan(middle);
});

test('the current tick still follows the chat after an older page is drawn', async () => {
  // 160 blocks, the latest 100 drawn. The rail holds 50 ticks: Earlier, then the person's latest 49.
  await render(<View state={chat(day(11, 80))} />);
  const current = () => ticks().findIndex((t) => t.getAttribute('aria-current'));
  await expect.poll(current).toBe(49);
  expect(below('u11-0')).toBeUndefined();

  viewport().scrollTop = 0;
  await expect.poll(() => below('u11-0')).toBeDefined();
  // The older page is in: at its top, the first tick is the current one and on screen.
  viewport().scrollTop = 0;
  await expect.poll(current).toBe(0);
  expect(ticks()[0].hasAttribute('data-in-view')).toBe(true);
  expect(ticks()[49].hasAttribute('data-in-view')).toBe(false);
});

test('a reply streaming into a running turn leaves the map as it was, until the turn ends', async () => {
  const state = chat(day(11, 3));
  const running: ChatState = {
    ...state,
    turns: {
      ...state.turns,
      next: { id: 'next', agent: 'mira', status: 'running', why: null, ref: 'next', position: 9 },
    },
    messages: [
      ...state.messages,
      {
        id: 'un',
        turn: 'next',
        time: new Date(2026, 10, 11, 15, 0).toISOString(),
        role: 'user',
        kind: 'text',
        body: 'One more',
      },
    ],
  };
  const screen = await render(<View state={running} />);
  await expect.poll(() => ticks().length).toBe(7);
  const before = ticks();
  // Every observer made while the reply streams: the map makes none again.
  const made: string[] = [];
  const Resize = window.ResizeObserver;
  const Mutation = window.MutationObserver;
  window.ResizeObserver = class extends Resize {
    constructor(callback: ResizeObserverCallback) {
      super(callback);
      made.push(new Error().stack?.includes('ChatMap') ? 'map' : 'other');
    }
  };
  window.MutationObserver = class extends Mutation {
    constructor(callback: MutationCallback) {
      super(callback);
      made.push(new Error().stack?.includes('ChatMap') ? 'map' : 'other');
    }
  };
  try {
    for (const partial of ['A reply', 'A reply that', 'A reply that grows']) {
      await screen.rerender(<View state={{ ...running, partial: { next: partial } }} />);
    }
    await expect.element(screen.getByText('A reply that grows')).toBeVisible();
    expect(made.filter((by) => by === 'map')).toEqual([]);
    const after = ticks();
    expect(after).toHaveLength(7);
    expect(after.every((t, i) => t === before[i])).toBe(true);
  } finally {
    window.ResizeObserver = Resize;
    window.MutationObserver = Mutation;
  }

  const done: ChatState = {
    ...running,
    turns: { ...running.turns, next: { ...running.turns.next, status: 'done' } },
    messages: [
      ...running.messages,
      {
        id: 'an',
        turn: 'next',
        time: new Date(2026, 10, 11, 15, 1).toISOString(),
        role: 'assistant',
        kind: 'text',
        body: 'A reply that grows',
      },
    ],
  };
  await screen.rerender(<View state={done} />);
  await expect.poll(() => names().at(-1)).toBe(`Mira · ${hhmm(15, 1)}`);
});

test('a search hit opened after a tick stays where it is once it is done', async () => {
  const state = chat(day(11, 6, tall));
  const screen = await render(<View state={state} />);
  await expect.poll(() => ticks().length).toBe(12);
  await userEvent.click(tick(`You · ${hhmm(10, 0)}`));
  await expect.poll(() => offTop('u11-1')).toBeLessThan(1);

  // The hit is shown, tinted, and the app clears it, as it does once a hit is in place.
  const onTarget = vi.fn();
  await screen.rerender(
    <View state={state} target={{ id: 'u11-4', at: performance.now() }} onTarget={onTarget} />,
  );
  await expect.poll(() => onTarget.mock.calls.length).toBe(1);
  const hit = document.querySelector('[data-message-id="u11-4"]') as HTMLElement;
  expect(hit.hasAttribute('data-search-hit')).toBe(true);
  const placed = below('u11-4');
  await screen.rerender(<View state={state} onTarget={onTarget} />);
  await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  expect(below('u11-4')).toBe(placed);
  expect(hit.hasAttribute('data-search-hit')).toBe(true);
});

test('a target whose message is not in the chat is let go, with a report, and holds nothing', async () => {
  const reports: string[] = [];
  vi.stubGlobal('fetch', async (_path: string, init?: RequestInit) => {
    reports.push(String(init?.body));
    return new Response('{}');
  });
  try {
    // While a hit is held, the replies with its text are marked as its passage: here, every answer.
    const target = { id: 'gone', text: 'Made-up answer', at: performance.now() };
    await render(<View state={chat(day(11, 3))} target={target} />);
    await expect
      .poll(() => reports.some((r) => r.includes('gathr.ui.search.error') && r.includes('not_drawn')))
      .toBe(true);
    await expect.poll(() => document.querySelectorAll('.found-part').length).toBe(0);
    expect(document.querySelectorAll('.reply')).toHaveLength(3);
  } finally {
    vi.unstubAllGlobals();
  }
});
