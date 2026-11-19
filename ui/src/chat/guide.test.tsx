// A thread's rule (0075): one 1px line beside its items, drawn piece by piece, one piece per item. The border colour is
// see-through in the dark theme, so a stretch where two pieces lay over each other read brighter and thicker (the
// owner's look of 2026-11-18). Each piece starts where the one above it ends: no overlap and no gap, whether the item
// above has the row under its bubble or not. Measured as the window lays it out. Made-up names and words only.
import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message, Turn } from '../api';
import { View } from './recorded';
import { type ChatState, empty } from './state';

const members = [
  { agent: 'a', name: 'Mira', role: 'supervisor' as const },
  { agent: 'b', name: 'Ann', role: 'member' as const },
  { agent: 'c', name: 'Bo', role: 'member' as const },
];
const time = (minute: number) => `2026-11-08T10:${String(minute).padStart(2, '0')}:00Z`;

/**
 * Mira's word, then six replies under it from Ann and Bo; Bo is still at work, so the thread is open. With `role` gathr,
 * the replies are Gathr's lines, which have no row under them.
 */
function state(role: Message['role'] = 'assistant'): ChatState {
  const turns: Record<string, Turn> = {};
  const messages: Message[] = [];
  for (let i = 0; i < 7; i++) {
    const id = `t${i}`;
    const agent = i === 0 ? 'a' : i % 2 ? 'b' : 'c';
    turns[id] = {
      id,
      agent,
      status: i === 6 ? 'running' : 'done',
      why: null,
      ref: id,
      position: i,
      created: time(i),
      topic: 'b:t0',
    };
    const body = i === 0 ? 'Made-up word for the team' : `Made-up reply ${i}, `.repeat(i % 3 ? 2 : 9).trim();
    messages.push({ id: `m${i}`, turn: id, time: time(i), role: i ? role : 'assistant', kind: 'text', body });
  }
  return { ...empty, members, turns, messages };
}

/** Each thread item's piece of the rule, top to bottom, in CSS px. */
function pieces() {
  const main = page.getByText('Made-up word for the team').element().closest('.said') as Element;
  const out: { what: string; top: number; bottom: number }[] = [];
  for (
    let el = main.nextElementSibling;
    el?.matches('.said[data-reply], [data-thread-item]');
    el = el.nextElementSibling
  ) {
    const box = el.getBoundingClientRect();
    const rule = getComputedStyle(el, '::before');
    out.push({
      what: `${el.className} ${el.textContent?.slice(0, 20)}`,
      top: box.top + Number.parseFloat(rule.top),
      bottom: box.bottom - Number.parseFloat(rule.bottom),
    });
  }
  return out;
}

for (const [which, role, feet] of [
  ['with the row under each reply', 'assistant', 3],
  ['with no row under the replies', 'gathr', 0],
] as const) {
  test(`a thread’s rule is one line, ${which}: each piece starts where the one above ends`, async () => {
    await page.viewport(1100, 900);
    await render(<View state={state(role)} actions={{ canReply: () => !!feet, reply: () => {} }} />);
    await expect.element(page.getByText('Show 3 earlier replies')).toBeVisible();
    const rows = [...document.querySelectorAll('.said[data-reply] .said-foot')].map((f) =>
      [...f.querySelectorAll('button')].map((b) => b.getAttribute('aria-label')).join(','),
    );
    expect(rows, rows.join('\n')).toHaveLength(feet);
    const all = pieces();
    // "Show earlier" and three replies.
    expect(all).toHaveLength(4);
    for (let i = 1; i < all.length; i++)
      expect(all[i].top - all[i - 1].bottom, `between ${all[i - 1].what} and ${all[i].what}`).toBeCloseTo(
        0,
        1,
      );
  });
}
