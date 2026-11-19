// The owner's rule for the threads (0075): the words are as wide as the message box, and the faces sit outside that
// width. Measured in a real browser at the chat's two widths: a wide pane, and the 360 column beside the side panel.
import { afterEach, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Composer } from '../components/Composer';
import { StateWord } from '../components/StateWord';
import type { Said } from '../tasks/rows';
import { chat, View } from './recorded';
import type { ChatState } from './state';

const LONG = 'Made-up long words that run past the end of one line, '.repeat(6).trim();

/** The recorded ask at work, with Ann's words in her task's thread long enough to fill a line, and a reply of the person's. */
function state(): ChatState {
  const s = chat('askAtWork');
  const ann = Object.values(s.turns).find((t) => t.topic === 't:2')?.id as string;
  const ask = s.messages[0];
  return {
    ...s,
    turns: { ...s.turns, [ann]: { ...s.turns[ann], status: 'done' } },
    messages: [
      ...s.messages,
      {
        id: 'ann-words',
        turn: ann,
        time: '2026-11-18T20:00:00Z',
        role: 'assistant',
        kind: 'text',
        body: LONG,
      },
      { ...ask, id: 'long-ask', body: LONG, turn: null, time: '2026-11-18T20:01:00Z' },
    ],
  };
}

const box = (el: Element | null | undefined) => (el as Element).getBoundingClientRect();

async function drawn(width: number) {
  await page.viewport(width + 200, 900);
  await render(
    <View state={state()} width={width}>
      <Composer draft="" onDraft={() => {}} onSend={() => {}} hint="Message Lead" />
    </View>,
  );
  await expect.element(page.getByText('Add the tags table')).toBeVisible();
  const composer = box(document.querySelector('.composer'));
  const main = (words: string) =>
    [...document.querySelectorAll('.thread-messages > .said:not([data-reply])')].find((m) =>
      m.textContent?.includes(words),
    ) as Element;
  const reply = [...document.querySelectorAll('.thread-messages > .said[data-reply]')].find((r) =>
    r.textContent?.includes('Made-up long words'),
  ) as Element;
  return { composer, main, reply };
}

afterEach(async () => {
  await page.viewport(1280, 900);
});

for (const width of [900, 360]) {
  test(`at ${width} wide, a main message's words start and end with the message box, its face outside`, async () => {
    const { composer, main } = await drawn(width);
    // The task's message fills the width; the person's long one does too.
    for (const words of ['Add the tags table', 'Made-up long words']) {
      const bubble = box(main(words).querySelector('.bubble'));
      expect(Math.abs(bubble.left - composer.left), words).toBeLessThan(1);
      expect(Math.abs(bubble.right - composer.right), words).toBeLessThan(1);
      const face = box(main(words).querySelector('.said-face > *'));
      expect(face.right, words).toBeLessThanOrEqual(composer.left - 11);
      // Never past the pane's own left edge.
      expect(face.left, words).toBeGreaterThanOrEqual(box(document.querySelector('.thread')).left);
    }
  });

  test(`at ${width} wide, a reply sits in its thread, its face and the rule inside, its words ending with the box`, async () => {
    const { composer, reply, main } = await drawn(width);
    const words = box(reply.querySelector('.bubble'));
    expect(Math.abs(words.right - composer.right)).toBeLessThan(1);
    // The rule at the box's left edge, then 12px, the 28px face and 10px: the words start 51px in.
    expect(Math.abs(words.left - composer.left - 51)).toBeLessThan(1);
    const face = box(reply.querySelector('.said-face > *'));
    expect(face.left).toBeGreaterThanOrEqual(composer.left);
    const rule = getComputedStyle(reply, '::before');
    expect(rule.borderLeftWidth).toBe('1px');
    expect(Math.abs(box(reply).left - composer.left)).toBeLessThan(1);
    // The fold line is in the row under the main message's bubble, at its left edge (0076).
    const fold = box(main('Add the tags table').querySelector('.said-foot .fold-line'));
    expect(Math.abs(fold.left - composer.left)).toBeLessThan(1);
  });
}

test('in the 360 column Supervisor leaves the name line, and the state keeps its place at its right', async () => {
  const { composer, main } = await drawn(360);
  const task = main('Add the tags table');
  const role = task.querySelector('.name-line-role') as HTMLElement;
  expect(getComputedStyle(role).display).toBe('none');
  const state = box(task.querySelector('.main-state'));
  expect(Math.abs(state.right - composer.right)).toBeLessThan(1);
  expect(task.querySelector('.main-state')?.textContent).toBe('Working');
});

test('in a wide pane the lead is named Supervisor on a main message', async () => {
  const { main } = await drawn(900);
  const role = main('Add the tags table').querySelector('.name-line-role') as HTMLElement;
  expect(getComputedStyle(role).display).not.toBe('none');
});

/** How far the text starts after the ink of the shape before it: the mark's own drawing, not its box. */
function gapOf(word: Element): number {
  const shapes = [...word.querySelectorAll('svg path, svg circle, svg rect, svg polyline')];
  const ink = Math.max(...shapes.map((s) => s.getBoundingClientRect().right));
  const text = [...word.childNodes].find((n) => n.nodeType === Node.TEXT_NODE) as Text;
  const range = document.createRange();
  range.selectNodeContents(text);
  return range.getBoundingClientRect().left - ink;
}

test('every state mark keeps one gap before its word, and Needs you is a small dot', async () => {
  const states: [string, Said][] = [
    ['Done', { done: true, word: 'Done' }],
    ['Needs you', { you: true, word: 'Needs you' }],
    ['Working', { mark: 'working' }],
    ['Waiting', { mark: 'waiting' }],
    ['Blocked', { mark: 'blocked' }],
    ['Stopped', { mark: 'stopped' }],
  ];
  await render(
    <div className="name-line" style={{ width: 400 }}>
      <span className="name-line-name">Made-up</span>
      {states.map(([name, said]) => (
        <span key={name} className="main-state">
          <StateWord said={said} className="main-state-word" />
        </span>
      ))}
    </div>,
  );
  const words = [...document.querySelectorAll('.main-state-word')];
  const gaps = words.map((w) => +gapOf(w).toFixed(1));
  for (const [i, [name]] of states.entries())
    expect(gaps[i], `${name}: ${gaps.join(', ')}`).toBeGreaterThanOrEqual(4.5);
  for (const [i, [name]] of states.entries())
    expect(gaps[i], `${name}: ${gaps.join(', ')}`).toBeLessThanOrEqual(5.5);
  // The mock-up's dot: 7.2px across, against the 12px marks.
  const dot = words[1].querySelector('svg circle')?.getBoundingClientRect();
  expect(Math.round(dot?.width ?? 0)).toBe(7);
});

test("the person's own message starts at the box's edge, and a plan's chip sits inside its bubble", async () => {
  const { composer, main } = await drawn(900);
  const ask = box(main('Add tags to notes').querySelector('.bubble'));
  expect(Math.abs(ask.left - composer.left)).toBeLessThan(1);
  const plan = main('version 1');
  const chip = box(plan.querySelector('.plan-chip'));
  const bubble = box(plan.querySelector('.bubble'));
  expect(Math.abs(bubble.left - composer.left)).toBeLessThan(1);
  expect(chip.left).toBeGreaterThan(bubble.left);
  expect(chip.right).toBeLessThan(composer.right);
});
