// The chat's foot (the owner's look of 2026-11-18): the message box alone, a little wider than the messages and their
// faces, with no band above it, so the chat scrolls behind it and is cut at its top edge; the New pill and the arrow to
// the foot float over the messages just above it. Measured as the window lays it out. Made-up names and words only.
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message, Read, Turn } from '../api';
import { ChatView } from './ChatView';

vi.mock('../live', () => ({ listen: () => () => {} }));

const members = [
  { agent: 'a', name: 'Mira', role: 'supervisor' as const },
  { agent: 'b', name: 'Ann', role: 'member' as const },
];
const made = (id: string, name: string) => ({
  id,
  name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto' as const,
  face_seed: name,
  gender: null,
  skin: null,
  background: null,
});
const time = (minute: number) => `2026-11-08T10:${String(minute).padStart(2, '0')}:00Z`;
const LONG = 'Made-up words that fill a few lines of the chat, '.repeat(6).trim();

/** Mira's long messages, each a main message with no replies; with `reads`, the person last saw the chat at 09:00. */
function service(reads?: Read[]) {
  const turns: Turn[] = [];
  const messages: Message[] = [];
  for (let i = 0; i < 18; i++) {
    const id = `t${i}`;
    turns.push({
      id,
      agent: 'a',
      status: 'done',
      why: null,
      ref: id,
      position: i + 1,
      created: time(i),
      topic: `b:${id}`,
    });
    messages.push({
      id: `m${i}`,
      turn: id,
      time: time(i),
      role: 'assistant',
      kind: 'text',
      body: `${LONG} ${i}`,
    });
  }
  const history = { chat: 'c', members, approvals: [], turns, messages, ...(reads && { reads }) };
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method && init.method !== 'GET') return new Response('{}');
    if (path.endsWith('/tasks')) return new Response(JSON.stringify({ tasks: [], waits: [], asks: [] }));
    if (path.endsWith('/c')) return new Response(JSON.stringify(history));
    return new Response('{}');
  });
}

beforeEach(() => {
  vi.spyOn(document, 'hasFocus').mockReturnValue(true);
  service();
});
afterEach(async () => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  await page.viewport(1280, 900);
});

const frames = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));

/** The chat `width` wide in a window `win` wide, 700 tall. */
async function open(width: number, win = width) {
  await page.viewport(win, 700);
  await render(
    <div style={{ height: 700, width, display: 'flex', flexDirection: 'column' }}>
      <style>{'.chat { flex: 1 }'}</style>
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat' }}
        agents={[made('a', 'Mira'), made('b', 'Ann')]}
        draft=""
        onDraft={() => {}}
        unsent={[]}
        onUnsent={() => {}}
        openSettings={() => {}}
        onNamed={() => {}}
        projects={[]}
        onProject={() => {}}
        onNewProject={() => {}}
        onStart={() => {}}
        pauses={{ pauses: [], usage: [] }}
        onPause={async () => {}}
      />
    </div>,
  );
  await expect.element(page.getByText(`${LONG} 17`)).toBeInTheDocument();
  await frames();
}

const viewport = () => document.querySelector('.thread-viewport') as HTMLElement;
const box = () =>
  (document.querySelector('.thread-footer > .composer') as HTMLElement).getBoundingClientRect();
const footer = () => document.querySelector('.thread-footer') as HTMLElement;
/** The first main message's face, and where the words of the column end. */
function words() {
  const face = (
    document.querySelector('.thread-messages > .said:not([data-reply]) .said-face') as HTMLElement
  ).getBoundingClientRect();
  const column = document.querySelector('.thread-column') as HTMLElement;
  const right =
    column.getBoundingClientRect().right - Number.parseFloat(getComputedStyle(column).paddingRight);
  return { face, right };
}
/** Scrolls the chat up from its foot, so messages lie behind the box. */
async function upBy(px: number) {
  const v = viewport();
  v.scrollTop = v.scrollHeight - v.clientHeight - px;
  await frames();
  await expect.poll(() => v.scrollHeight - v.clientHeight - v.scrollTop).toBeGreaterThan(px - 2);
}
const inChat = (x: number, y: number) => !!document.elementFromPoint(x, y)?.closest('.thread-messages');

test('at 1440 the box reaches 8px past the faces on the left, and as far past the words on the right', async () => {
  await open(1440);
  const b = box();
  const { face, right } = words();
  expect(b.left).toBeCloseTo(face.left - 8, 0);
  expect(b.right).toBeCloseTo(right + 56, 0);
  expect(b.width).toBeCloseTo(752, 0);
  // Centred as the column is.
  const v = viewport().getBoundingClientRect();
  expect(b.left - v.left).toBeCloseTo(v.left + viewport().clientWidth - b.right, 0);
});

for (const [width, win] of [
  [1000, 1000],
  [740, 1000],
  [360, 1000],
] as const) {
  test(`a ${width} wide chat: the box takes the width there is, 16px in each side, with no sideways scroll`, async () => {
    await open(width, win);
    const v = viewport();
    const left = v.getBoundingClientRect().left;
    const b = box();
    const room = v.clientWidth - 32;
    expect(b.width).toBeCloseTo(Math.min(752, room), 0);
    expect(b.left).toBeGreaterThanOrEqual(left + 16 - 0.5);
    expect(b.right).toBeLessThanOrEqual(left + v.clientWidth - 16 + 0.5);
    expect(v.scrollWidth).toBeLessThanOrEqual(v.clientWidth);
    expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(win);
  });
}

test('the foot holds the box alone: no status line, no line at the foot, no slot kept for them', async () => {
  await open(1440);
  expect(document.querySelector('.status-slot, .status-line, .foot-line')).toBeNull();
  // The footer is the box and the strip under it.
  expect(footer().offsetHeight).toBe(
    (document.querySelector('.thread-footer > .composer') as HTMLElement).offsetHeight + 20,
  );
});

test('nothing is painted above the box: a message just above its top edge shows, and nothing fades over it', async () => {
  await open(1440);
  await upBy(400);
  const b = box();
  const x = b.left + b.width / 2;
  // A message's words or the space between messages, never a band.
  expect(inChat(x, b.top - 2)).toBe(true);
  expect(inChat(b.left + 60, b.top - 1)).toBe(true);
  expect(getComputedStyle(footer(), '::before').content).toMatch(/^(none|normal)$/);
});

test('a message scrolled behind the box shows neither beside it nor under it', async () => {
  await open(1440);
  await upBy(400);
  const b = box();
  const { face } = words();
  // Under the box: at a face's place and at the middle.
  expect(inChat(face.left + 4, b.bottom + 6)).toBe(false);
  expect(inChat(b.left + b.width / 2, b.bottom + 6)).toBe(false);
  // Beside it, at its height.
  expect(inChat(b.left - 4, b.top + 10)).toBe(false);
  expect(inChat(b.right + 4, b.top + 10)).toBe(false);
  // Behind it.
  expect(inChat(face.left + 4, b.top + 10)).toBe(false);
});

test('at the chat’s foot the last message’s row of replies and buttons clears the box', async () => {
  await open(1440);
  const v = viewport();
  await expect.poll(() => v.scrollHeight - v.clientHeight - v.scrollTop).toBeLessThan(2);
  const last = [...document.querySelectorAll('.thread-messages > .said')].at(-1) as HTMLElement;
  const row = last.querySelector('.said-foot') as HTMLElement;
  expect(row, 'the last message has its row').not.toBeNull();
  // The owner, 2026-11-19: the row was cut off by the box on the last message.
  expect(box().top - row.getBoundingClientRect().bottom).toBeGreaterThanOrEqual(8);
});

test('the arrow to the foot sits just above the box, its right edge in line with the box’s', async () => {
  await open(1440);
  await upBy(400);
  const arrow = page.getByRole('button', { name: 'Scroll to bottom' });
  await expect.element(arrow).toBeVisible();
  const a = arrow.element().getBoundingClientRect();
  const b = box();
  expect(a.right).toBeCloseTo(b.right, 0);
  expect(a.bottom).toBeLessThanOrEqual(b.top);
  expect(b.top - a.bottom).toBeLessThan(16);
});

test('the New pill floats centred just above the box, as high as the arrow, and keeps no room of its own', async () => {
  // The window in the back: every message shows as new.
  vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  service([{ topic: '', seen_us: Date.parse('2026-11-08T09:00:00Z') * 1000, folded: null }]);
  await open(1440);
  const pill = page.getByRole('button', { name: /new messages/ });
  await expect.element(pill).toBeVisible();
  await expect.element(pill).toHaveTextContent('18 New');
  const p = pill.element().getBoundingClientRect();
  const b = box();
  expect(p.left + p.width / 2).toBeCloseTo(b.left + b.width / 2, 0);
  expect(p.bottom).toBeLessThanOrEqual(b.top);
  expect(b.top - p.bottom).toBeLessThan(16);
  expect(p.height).toBe(24);
  expect(footer().offsetHeight).toBe(
    (document.querySelector('.thread-footer > .composer') as HTMLElement).offsetHeight + 20,
  );
  const border = getComputedStyle(pill.element());
  expect(border.fontSize).toBe('12.5px');
  expect(border.fontWeight).toBe('550');
});
