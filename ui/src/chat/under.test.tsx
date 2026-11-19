// The row under a message's bubble (0076, the owner's call of 2026-11-18): the fold line, the thumbs-up count, then
// Reply and Like at the right, shown only while the message is pointed at or holds keyboard focus, in a spot always
// kept for them, ending at the bubble's bottom right edge. Measured in a real browser at 900 wide and in the 360
// column. Made-up names and words only.
import { afterEach, expect, test } from 'vitest';
import { page, server, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message, Turn } from '../api';
import { Composer } from '../components/Composer';
import { View } from './recorded';
import { type ChatState, empty } from './state';

const LONG = 'Made-up long words that run past the end of one line, '.repeat(6).trim();
const members = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' as const },
  { agent: 'b', name: 'Ann', role: 'member' as const },
];
const turn = (id: string, agent: string, position: number, topic: string): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  position,
  created: `2026-11-08T10:0${position}:00Z`,
  topic,
});
const said = (
  id: string,
  turnId: string,
  minute: number,
  body: string,
  more: Partial<Message> = {},
): Message => ({
  id,
  turn: turnId,
  time: `2026-11-08T10:${String(minute).padStart(2, '0')}:00Z`,
  role: 'assistant',
  kind: 'text',
  body,
  ...more,
});

/**
 * A team chat: the person's ask with the lead's answer under it (its state word at its name line's right); the lead's
 * short "Yes." with Ann's long reply under it and Ann's thumbs-up on it; and Ann's long message with steps.
 */
function state(): ChatState {
  return {
    ...empty,
    members,
    turns: {
      t1: turn('t1', 'a', 1, 'm:p1'),
      t2: turn('t2', 'a', 2, 'b:t2'),
      // Ann still at work: her thread isn't handled, so it starts open.
      t3: { ...turn('t3', 'b', 3, 'b:t2'), status: 'running' },
      t4: turn('t4', 'b', 4, 'b:t4'),
    },
    messages: [
      said('p1', 't1', 1, 'Plan the made-up walk', { role: 'user' }),
      said('l1', 't1', 2, 'Made-up plan words'),
      said('yes-step', 't2', 3, '', {
        kind: 'tool',
        body: { name: 'Bash', input: { command: 'made-up' }, status: 'done' } as unknown as string,
      }),
      said('yes', 't2', 3, 'Yes.'),
      said('ann', 't3', 4, LONG),
      said('step', 't4', 5, '', {
        kind: 'tool',
        body: { name: 'Bash', input: { command: 'made-up' }, status: 'done' } as unknown as string,
      }),
      said('long', 't4', 6, LONG),
    ],
    reactions: [{ message: 'yes', agent: 'b', time: '2026-11-08T10:05:00Z' }],
    asks: [
      {
        id: 'p1',
        title: 'Plan the made-up walk',
        size: 'repair',
        by: 'a',
        sized_in: null,
        held: false,
        open: false,
      },
    ],
  };
}

const box = (el: Element | null | undefined) => (el as Element).getBoundingClientRect();
const rect = (b: DOMRect) => [b.left, b.top, b.right, b.bottom].map((n) => Math.round(n * 10) / 10);
const apart = (a: DOMRect, b: DOMRect) =>
  a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top;

async function drawn(width: number) {
  await page.viewport(width + 200, 900);
  await render(
    <View state={state()} width={width} actions={{ canReply: () => true, reply: () => {} }}>
      <Composer draft="" onDraft={() => {}} onSend={() => {}} hint="Message Made-up Lead" />
    </View>,
  );
  await expect.element(page.getByText('Yes.', { exact: true })).toBeVisible();
  const message = (words: string, reply = false) =>
    [
      ...document.querySelectorAll(
        `.thread-messages > .said${reply ? '[data-reply]' : ':not([data-reply])'}`,
      ),
    ].find((m) => m.querySelector('.bubble')?.textContent?.startsWith(words)) as HTMLElement;
  return { message };
}

const buttons = (m: Element) => m.querySelector('.said-foot .message-actions') as HTMLElement;
const shown = (m: Element) => Number(getComputedStyle(buttons(m)).opacity);

afterEach(async () => {
  await page.viewport(1280, 900);
  await userEvent.unhover(document.body);
});

for (const width of [900, 360]) {
  test(`at ${width} wide, the buttons show only while the message is pointed at or holds focus`, async () => {
    const { message } = await drawn(width);
    const yes = message('Yes.');
    expect(buttons(yes), 'the row under the bubble holds the buttons').not.toBeNull();
    expect(yes.querySelector('.bubble .message-actions'), 'no corner row').toBeNull();
    expect(shown(yes)).toBe(0);
    await userEvent.hover(yes.querySelector('.bubble') as HTMLElement);
    expect(shown(yes)).toBe(1);
    await userEvent.unhover(yes);
    await userEvent.hover(document.querySelector('.composer') as HTMLElement);
    expect(shown(yes)).toBe(0);
    // A press with the pointer leaves focus on what was pressed: the buttons still go when the pointer does.
    const fold = yes.querySelector('.fold-line') as HTMLElement;
    await userEvent.click(fold);
    await userEvent.hover(document.querySelector('.composer') as HTMLElement);
    await expect.poll(() => shown(yes), { message: 'after a press on the fold line' }).toBe(0);
    expect(getComputedStyle(yes.querySelector('.steps-button') as Element).opacity, 'the steps button').toBe(
      '0',
    );
    // Safari's Tab skips buttons unless the person turns that on; Option+Tab reaches them.
    fold.focus();
    await userEvent.keyboard(server.browser === 'webkit' ? '{Alt>}{Tab}{/Alt}' : '{Tab}');
    // Tab moves on along the row, to the thumbs-up count.
    await expect.poll(() => document.activeElement?.className).toBe('react');
    expect(shown(yes)).toBe(1);
  });

  test(`at ${width} wide, the buttons end at a wide bubble's bottom right edge`, async () => {
    const { message } = await drawn(width);
    // A main message and a reply in a thread, each filling the width.
    for (const [m, which] of [
      [message('Made-up long words'), 'main'],
      [message('Made-up long words', true), 'reply'],
    ] as const) {
      const bubble = box(m.querySelector('.bubble'));
      const row = box(buttons(m));
      expect(Math.abs(row.right - bubble.right), which).toBeLessThan(1);
      expect(row.top, `${which}: under the bubble`).toBeGreaterThanOrEqual(bubble.bottom);
    }
  });

  test(`at ${width} wide, on a short bubble the buttons follow its fold line and count, covering neither`, async () => {
    const { message } = await drawn(width);
    const yes = message('Yes.');
    const fold = box(yes.querySelector('.said-foot .fold-line'));
    const count = box(yes.querySelector('.said-foot .react'));
    const row = box(buttons(yes));
    const bubble = box(yes.querySelector('.bubble'));
    expect(bubble.right, 'a short bubble').toBeLessThan(row.left);
    expect(fold.right).toBeLessThanOrEqual(count.left);
    expect(count.right).toBeLessThanOrEqual(row.left);
    expect(Math.abs(fold.left - bubble.left)).toBeLessThan(1);
    // The row is one line: the fold line, the count and the buttons share it.
    expect(Math.abs(fold.top + fold.height / 2 - (row.top + row.height / 2))).toBeLessThan(2);
  });

  test(`at ${width} wide, nothing moves when the buttons show`, async () => {
    const { message } = await drawn(width);
    const yes = message('Yes.');
    const parts = () => {
      const next = yes.nextElementSibling as Element;
      return [
        yes.querySelector('.bubble'),
        yes.querySelector('.name-line'),
        yes.querySelector('.fold-line'),
        yes.querySelector('.react'),
        next,
        message('Made-up long words'),
      ].map((e) => rect(box(e)));
    };
    const hidden = parts();
    await userEvent.hover(yes.querySelector('.bubble') as HTMLElement);
    expect(shown(yes)).toBe(1);
    expect(parts()).toEqual(hidden);
  });

  test(`at ${width} wide, the buttons cover no state word, steps button, name line, words or next message`, async () => {
    const { message } = await drawn(width);
    const all = [...document.querySelectorAll('.thread-messages > .said')];
    let checked = 0;
    for (const m of all) {
      // Wherever the buttons are: the corner row lay over the name line.
      const row = m.querySelector('.message-actions');
      if (!row) continue;
      checked++;
      const r = box(row);
      const next = m.nextElementSibling;
      const others = [
        ...m.querySelectorAll('.name-line, .main-state, .steps-button, .bubble'),
        ...(next ? [next] : []),
      ];
      for (const o of others)
        expect(apart(r, box(o)), `${m.textContent?.slice(0, 30)} over ${o.className}`).toBe(true);
    }
    // The person's ask, the lead's answer, "Yes.", Ann's reply and Ann's long message.
    expect(checked).toBeGreaterThanOrEqual(4);
    expect(message('Plan the made-up walk').querySelector('.main-state')).not.toBeNull();
    expect(message('Made-up long words').querySelector('.steps-button')).not.toBeNull();
  });
}
