// Reply keeps the view at the box it opens (0076): on a main message with no replies, a waiting question or not, in a
// chat tall enough to scroll, the box shows under the message with focus in it and the message stays in view, whether
// the view is far from the chat's foot, a few hundred pixels above it, or at it. Made-up names and words only.
import { afterEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message, Turn } from '../api';
import { ChatView } from './ChatView';

const members = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' as const },
  { agent: 'b', name: 'Ann', role: 'member' as const },
];
const made = (id: string, name: string) => ({
  id,
  name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto' as const,
  face_seed: 'Made-up',
  gender: null,
  skin: null,
  background: null,
});
const time = (minute: number) => `2026-11-08T10:${String(minute).padStart(2, '0')}:00Z`;
const LONG = 'Made-up words that fill a few lines of the chat, '.repeat(5).trim();

/** What each of the lead's messages says: two waiting questions, a short and a plain one, and long ones between. */
const WORDS: Record<number, string> = {
  4: 'Made-up question words',
  6: 'Made-up plain words',
  12: 'Made-up short words',
  13: 'Made-up last question',
};
const ASKED: Record<number, number> = { 4: 9, 13: 10 };

/** The lead's messages tied to nothing (main messages with no replies), its two questions among them. */
function history() {
  const turns: Turn[] = [];
  const messages: Message[] = [];
  for (let i = 0; i < 18; i++) {
    const id = `t${i}`;
    const topic = ASKED[i] ? `t:${ASKED[i]}` : `b:${id}`;
    turns.push({
      id,
      agent: 'a',
      status: 'done',
      why: null,
      ref: id,
      position: i + 1,
      created: time(i),
      topic,
    });
    const body = WORDS[i] ?? `${LONG} ${i}`;
    messages.push({ id: `m${i}`, turn: id, time: time(i), role: 'assistant', kind: 'text', body });
  }
  return { chat: 'c', members, approvals: [], turns, messages };
}
const question = (number: number, turn: number, what: string) => ({
  number,
  chat: 'c',
  giver: 'a',
  owner: null,
  given_in: `t${turn}`,
  kind: 'person',
  what,
  state: 'ready',
  reason: null,
  after: [],
  created_us: Date.parse(time(turn)) * 1000,
  form: 'choice',
  choices: [{ label: 'Made-up yes' }, { label: 'Made-up no' }],
  topic: `t:${number}`,
});
const tasks = [
  question(9, 4, 'Made-up question: the long way?'),
  question(10, 13, 'Made-up question: the short way?'),
];

/** The chat's history comes a moment after it opens, as from the service: the chat is drawn empty first. */
function service() {
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST') return new Response('{}');
    if (path.endsWith('/tasks'))
      return new Response(JSON.stringify({ tasks, waits: [], asks: [], plans: [], caught: [] }));
    if (path.endsWith('/c')) {
      await new Promise((r) => setTimeout(r, 200));
      return new Response(JSON.stringify(history()));
    }
    return new Response('{}');
  });
}

afterEach(() => vi.unstubAllGlobals());

const frames = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));

async function open() {
  service();
  await page.viewport(1100, 700);
  await render(
    <div style={{ height: 700, display: 'flex', flexDirection: 'column' }}>
      {/* As the app's grid gives it: the chat fills its cell from the first draw, before its history comes. */}
      <style>{'.chat { flex: 1 }'}</style>
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat' }}
        agents={[made('a', 'Made-up Lead'), made('b', 'Ann')]}
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
        answers={{ drafts: {}, onDraft: () => {}, onSend: async () => null }}
      />
    </div>,
  );
  await expect.element(page.getByText('Made-up last question')).toBeInTheDocument();
  await frames();
}

const viewport = () => document.querySelector('.thread-viewport') as HTMLElement;
const said = (words: string) =>
  page.getByText(words, { exact: true }).element().closest('.said') as HTMLElement;
const inView = (el: Element) => {
  const v = viewport().getBoundingClientRect();
  const r = el.getBoundingClientRect();
  return r.bottom > v.top && r.top < v.bottom;
};
/** How far the view is above the chat's foot. */
const fromFoot = () => {
  const v = viewport();
  return v.scrollHeight - v.clientHeight - v.scrollTop;
};

/** Scrolls so the message's top sits just under the top of the view. */
async function scrollTo(m: HTMLElement) {
  const v = viewport();
  v.scrollTop += m.getBoundingClientRect().top - v.getBoundingClientRect().top - 40;
  await frames();
}

/** Reply on the message: the box once drawn, after any scroll its drawing set off. */
async function reply(m: HTMLElement) {
  await userEvent.hover(m.querySelector('.bubble') as HTMLElement);
  await userEvent.click(m.querySelector('.message-actions [aria-label="Reply"]') as HTMLElement);
  await expect.element(page.getByRole('textbox', { name: 'Reply' })).toHaveFocus();
  await frames();
  return document.querySelector('.thread-box') as HTMLElement;
}

/**
 * Reply on the message keeps the view where it was: the message doesn't move, and its box shows whole under it, above
 * the message box at the foot.
 */
async function keepsTheView(words: string) {
  const before = said(words).getBoundingClientRect().top;
  const box = await reply(said(words));
  const moved = said(words).getBoundingClientRect().top - before;
  expect(Math.abs(moved), `the message moved ${moved}px (${fromFoot()}px above the foot)`).toBeLessThan(2);
  const footer = (document.querySelector('.thread-footer') as HTMLElement).getBoundingClientRect();
  const b = box.getBoundingClientRect();
  expect(b.top, 'the box under the top of the view').toBeGreaterThanOrEqual(
    viewport().getBoundingClientRect().top,
  );
  expect(b.bottom, 'the box above the message box').toBeLessThanOrEqual(footer.top);
}
for (const [what, words] of [
  ['a waiting question', 'Made-up question words'],
  ['a main message', 'Made-up plain words'],
] as const) {
  test(`Reply on ${what} with no replies, far from the foot, keeps the view at its box`, async () => {
    await open();
    await scrollTo(said(words));
    expect(fromFoot(), 'far from the foot').toBeGreaterThan(800);
    await keepsTheView(words);
  });
}

/** A few hundred pixels above the foot: more than the message box is tall, less than the chat's view. */
for (const [what, words] of [
  ['a waiting question', 'Made-up last question'],
  ['a main message', 'Made-up short words'],
] as const) {
  test(`Reply on ${what} with no replies, a few hundred pixels above the foot, keeps the view at its box`, async () => {
    await open();
    await scrollTo(said(words));
    const footer = (document.querySelector('.thread-footer') as HTMLElement).offsetHeight;
    expect(fromFoot(), 'above the foot by more than the message box').toBeGreaterThan(footer + 50);
    expect(fromFoot()).toBeLessThan(viewport().clientHeight);
    await keepsTheView(words);
  });
}

test('Reply on the last main message, with the chat at its foot, keeps the box in view', async () => {
  await open();
  const v = viewport();
  v.scrollTop = v.scrollHeight;
  await frames();
  expect(fromFoot()).toBeLessThan(2);
  const box = await reply(said(`${LONG} 17`));
  expect(inView(box)).toBe(true);
});
