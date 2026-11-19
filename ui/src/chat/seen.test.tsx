// Where the person left off (0077), in the chat as the window draws it, the service at the edge: fetch answers as it
// would, keeping the marks it is sent, and the live socket is a stand-in. The New line, the blue counts, each reply's
// New, the New pill above the message box, the folds, and what a chat with no marks opens as. Pointer and keyboard.
import './renderWatch';
import { useState } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, History, Member, Message, Read, Task, Turn } from '../api';
import type { LiveEvent } from '../live';
import { ChatView } from './ChatView';
import { WRITE_MS } from './reads';
import { watchRenders } from './renderWatch';
import '../app.css';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('../live', () => ({
  listen: (listener: (event: LiveEvent) => void) => {
    live.add(listener);
    return () => live.delete(listener);
  },
}));
const send = (event: LiveEvent) => {
  for (const on of live) on(event);
};

beforeEach(async () => {
  await page.viewport(1100, 700);
  // The tests' page sits in a frame that never has focus: the window is in front here.
  vi.spyOn(document, 'hasFocus').mockReturnValue(true);
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  live.clear();
});

const members: Member[] = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' },
  { agent: 'ann', name: 'Ann', role: 'member' },
  { agent: 'bo', name: 'Bo', role: 'member' },
  { agent: 'tam', name: 'Tam', role: 'member' },
];
const agents: Agent[] = members.map((m) => ({
  id: m.agent,
  name: m.name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto',
  face_seed: m.name,
  gender: null,
  skin: null,
  background: null,
}));

/** Yesterday at `h:m:s`, so nothing is later than now. */
const at = (h: number, m: number, s = 0) => {
  const d = new Date(Date.now() - 86_400_000);
  d.setHours(h, m, s, 0);
  return d.toISOString();
};
const us = (time: string) => Date.parse(time) * 1000;

let position = 0;
const turn = (id: string, agent: string, topic: string, time: string, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  created: time,
  position: position++,
  topic,
  ...more,
});
const words = (
  id: string,
  turnId: string,
  body: string,
  time: string,
  role: Message['role'] = 'assistant',
) => ({
  id,
  turn: turnId,
  time,
  role,
  kind: 'text' as const,
  body,
});
const task = (number: number, owner: string, what: string, state: Task['state'], time: string): Task => ({
  number,
  chat: 'c',
  giver: 'mira',
  owner,
  given_in: 'ask',
  kind: 'work',
  what,
  why: `Made-up why of ${what}`,
  state,
  reason: null,
  after: [],
  created_us: us(time),
  topic: `t:${number}`,
});

const ANN_NEW = 'Adding up works: 2 onions and 1 onion make 3 onions.';

/**
 * A made-up morning, as `#new` draws it: the person's ask, three tasks (Tam's done, Ann's and Bo's at work), ten more
 * asks of the person's, each answered, then Mira's word at 10:05. Ann's last reply came at 10:08 and Bo's thread has
 * three replies after 09:58. `without` leaves turns out, to send them live; `running` turns are still at work.
 */
function morning(without: string[] = [], running: string[] = []) {
  position = 0;
  const turns: Turn[] = [];
  const messages: Message[] = [];
  const add = (t: Turn, ...said: Message[]) => {
    if (without.includes(t.id)) return;
    turns.push(running.includes(t.id) ? { ...t, status: 'running' } : t);
    messages.push(...said);
  };
  add(
    turn('ask', 'mira', 'm:u-ask', at(9, 30)),
    words('u-ask', 'ask', 'Plan the made-up shopping list', at(9, 30), 'user'),
    words('w-ask', 'ask', 'Three tasks out: Tam, Ann and Bo.', at(9, 31)),
  );
  add(
    turn('ann1', 'ann', 't:2', at(9, 36), { task: 2 }),
    words('w-ann1', 'ann1', 'Starting with the adding up.', at(9, 36)),
  );
  add(
    turn('bo1', 'mira', 't:3', at(9, 40)),
    words('w-bo1', 'bo1', 'Bo, the planner screen is yours.', at(9, 40)),
  );
  for (let i = 0; i < 10; i++) {
    const id = `f${i}`;
    add(
      turn(id, 'mira', `m:u-${id}`, at(9, 41 + i)),
      words(`u-${id}`, id, `Made-up side ask ${i}`, at(9, 41 + i), 'user'),
      words(`w-${id}`, id, `Made-up answer ${i}`, at(9, 41 + i, 30)),
    );
  }
  add(
    turn('tam', 'tam', 't:1', at(9, 52), { task: 1 }),
    words('w-tam', 'tam', 'Units tidied in every recipe.', at(9, 52)),
  );
  add(
    turn('bo2', 'bo', 't:3', at(10, 1), { task: 3 }),
    words('w-bo2', 'bo2', 'The screen is up in Planner.', at(10, 1)),
  );
  add(turn('ann2', 'ann', 't:3', at(10, 2)), words('w-ann2', 'ann2', 'Nice. The shape is final.', at(10, 2)));
  add(
    turn('mira3', 'mira', 't:3', at(10, 3)),
    words('w-mira3', 'mira3', 'Rev, PR 22 is yours to review.', at(10, 3)),
  );
  add(
    turn('wake', 'mira', 'b:wake', at(10, 5)),
    words('w-wake', 'wake', 'Bo’s screen lists ingredients in recipe order.', at(10, 5)),
  );
  add(turn('ann3', 'ann', 't:2', at(10, 8), { task: 2 }), words('w-ann3', 'ann3', ANN_NEW, at(10, 8)));
  const tasks = [
    task(1, 'tam', 'Tidy units in old recipes', 'done', at(9, 32)),
    task(2, 'ann', 'Add up ingredients', 'working', at(9, 32, 30)),
    task(3, 'bo', 'Shopping list screen', 'working', at(9, 33)),
  ];
  return { turns, messages, tasks };
}

/** The person was last here at 09:58, with Bo's thread folded; everything before that is seen. */
const left = (more: Read[] = []): Read[] => [
  { topic: '', seen_us: us(at(9, 58)), folded: null },
  { topic: 't:1', seen_us: us(at(9, 58)), folded: null },
  { topic: 't:2', seen_us: us(at(9, 58)), folded: null },
  { topic: 't:3', seen_us: us(at(9, 58)), folded: true },
  { topic: 'm:u-ask', seen_us: us(at(9, 58)), folded: null },
  ...Array.from({ length: 10 }, (_, i) => ({ topic: `m:u-f${i}`, seen_us: us(at(9, 58)), folded: null })),
  ...more,
];

/** A service with the morning's chat, keeping the marks it is sent as the service does: never back, the fold latest. */
function service(day: ReturnType<typeof morning>, reads: Read[]) {
  const kept = new Map(reads.map((r) => [r.topic, { ...r }]));
  const puts: Read[][] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'PUT' && path.endsWith('/reads')) {
      const sent = (JSON.parse(String(init.body)) as { reads: Partial<Read>[] }).reads;
      puts.push(sent as Read[]);
      for (const r of sent) {
        const was = kept.get(r.topic as string) ?? { topic: r.topic as string, seen_us: 0, folded: null };
        kept.set(was.topic, {
          topic: was.topic,
          seen_us: Math.max(was.seen_us, r.seen_us ?? 0),
          folded: r.folded === undefined ? was.folded : r.folded,
        });
      }
      return new Response(JSON.stringify({ reads: [] }));
    }
    if (path.endsWith('/waiting')) return new Response('{}');
    if (path.endsWith('/tasks'))
      return new Response(JSON.stringify({ tasks: day.tasks, waits: [], asks: [] }));
    const history: History = {
      chat: 'c',
      members,
      approvals: [],
      turns: day.turns,
      messages: day.messages,
      reads: [...kept.values()],
    };
    return new Response(JSON.stringify(history));
  });
  return { puts, kept };
}

/** The chat in the window; `n` changes to open it again, as a reload does. */
function Chat({ n = 0 }: { n?: number }) {
  const [draft, setDraft] = useState('');
  return (
    <div className="app" style={{ height: 640 }}>
      <ChatView
        key={n}
        chat={{ id: 'c', title: 'Made-up morning' }}
        agents={agents}
        draft={draft}
        onDraft={setDraft}
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
    </div>
  );
}

const mainWith = (text: string) =>
  [...document.querySelectorAll('.thread-messages > .said:not([data-reply])')].find((el) =>
    el.textContent?.includes(text),
  ) as HTMLElement | undefined;
const replyWith = (text: string) =>
  [...document.querySelectorAll('.thread-messages > .said[data-reply]')].find((el) =>
    el.textContent?.includes(text),
  ) as HTMLElement | undefined;
const foldOf = (main: Element | undefined) => main?.querySelector<HTMLButtonElement>('.said-foot .fold-line');
const pill = () => document.querySelector<HTMLButtonElement>('.new-pill');
/** Which way the pill's arrow points. */
const arrow = () => {
  const svg = pill()?.querySelector('svg');
  return svg?.classList.contains('lucide-arrow-up')
    ? 'up'
    : svg?.classList.contains('lucide-arrow-down')
      ? 'down'
      : '';
};
/** The newest mark sent for a thread, in µs. */
const sentFor = (puts: Read[][], topic: string) =>
  Math.max(0, ...puts.flat().flatMap((r) => (r.topic === topic && r.seen_us ? [r.seen_us] : [])));
/**
 * A wait for a write to the service: the window may hold one for up to WRITE_MS after its last, which is as long as a
 * poll waits unless told. Under load that was a race, seen in WebKit.
 */
const written = <T,>(read: () => T) => expect.poll(read, { timeout: 3 * WRITE_MS });
const attention = () => {
  const probe = document.createElement('span');
  probe.style.color = getComputedStyle(document.documentElement).getPropertyValue('--attention').trim();
  document.body.append(probe);
  const color = getComputedStyle(probe).color;
  probe.remove();
  return color;
};

test('back in a chat, the New line sits above the first main message not seen, and nowhere else', async () => {
  service(morning(), left());
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
  const lines = document.querySelectorAll('.new-line');
  expect(lines.length).toBe(1);
  const next = lines[0].nextElementSibling;
  expect(next?.textContent).toContain('Bo’s screen lists ingredients in recipe order.');
  expect(lines[0].textContent).toBe('New');
  expect(lines[0].getAttribute('role')).toBe('separator');
});

test('a folded thread counts its new replies in blue; opened, each reply not seen says New', async () => {
  const { puts } = service(morning(), left());
  await render(<Chat />);
  const bo = () => mainWith('Shopping list screen');
  await expect.poll(() => foldOf(bo())?.textContent).toBe('3 new replies');
  // A chat opens at its foot, a moment after it is drawn: going up the page before that, the view was pulled back down
  // and the opened thread was never in it (seen in WebKit, one run in three).
  await expect
    .poll(() => {
      const view = document.querySelector('.thread-viewport') as HTMLElement;
      return view.scrollHeight - view.scrollTop - view.clientHeight < 2;
    })
    .toBe(true);
  const fold = foldOf(bo()) as HTMLButtonElement;
  expect(fold.getAttribute('aria-expanded')).toBe('false');
  expect(getComputedStyle(fold).color).toBe(attention());

  fold.focus();
  // Focus takes the view up to the line, and the chat knows at once that it has left its foot (useFocusUp): its down
  // arrow shows. Before that the chat learnt it from the scroll, frames later in WebKit, and anything drawn at the foot
  // meanwhile took the view back down (the arrow never showed, once on macOS in CI); the next test forces that.
  await expect.poll(() => document.querySelector<HTMLButtonElement>('.thread-bottom')?.disabled).toBe(false);
  await userEvent.keyboard('{Enter}');
  await expect.poll(() => foldOf(bo())?.textContent).toBe('Collapse replies');
  const said = [
    'The screen is up in Planner.',
    'Nice. The shape is final.',
    'Rev, PR 22 is yours to review.',
  ];
  for (const text of said) {
    await expect.poll(() => replyWith(text)?.querySelector('.name-line-new')?.textContent).toBe('New');
  }
  // The opened thread scrolls its last reply into view, and in view it is seen: only then is none left new. Showing
  // the earlier reply moves the view up to it, so a test that went on at once left the last two not seen.
  await written(() => sentFor(puts, 't:3')).toBeGreaterThanOrEqual(us(at(10, 3)));
  await page.getByRole('button', { name: 'Show 1 earlier reply' }).click();
  await expect.poll(() => !!replyWith('Bo, the planner screen is yours.')).toBe(true);
  expect(replyWith('Bo, the planner screen is yours.')?.querySelector('.name-line-new')).toBeNull();
  // Seen once opened: folded again, it counts them all, plainly.
  await (foldOf(bo()) as HTMLButtonElement).click();
  await expect.poll(() => foldOf(bo())?.textContent).toBe('4 replies');
  expect(getComputedStyle(foldOf(bo()) as HTMLElement).color).not.toBe(attention());
});

test('keyboard focus up the chat stays in view while the chat grows at its foot, and Enter opens the thread there', async () => {
  service(morning(), left());
  await render(<Chat />);
  const bo = () => mainWith('Shopping list screen');
  await expect.poll(() => foldOf(bo())?.textContent).toBe('3 new replies');
  const view = document.querySelector('.thread-viewport') as HTMLElement;
  await expect.poll(() => view.scrollHeight - view.scrollTop - view.clientHeight < 2).toBe(true);
  // Between the chat's top and the message box, to the pixel.
  const inView = (el: Element | null | undefined) => {
    const box = (el as Element).getBoundingClientRect();
    const top = view.getBoundingClientRect().top;
    const bottom = (view.querySelector('.thread-footer') as Element).getBoundingClientRect().top;
    return box.top >= top - 1 && box.bottom <= bottom + 1;
  };
  const fold = foldOf(bo()) as HTMLButtonElement;
  expect(inView(fold)).toBe(false);

  // The chat grows at its foot before it has learnt, from the scroll, that focus took the view up: it followed its foot
  // and took the view back down, away from what had focus. In WebKit the focus scrolls frames later, so anything drawn
  // at the foot meanwhile (a streamed reply) did it; here the foot grows in the same moment, so every engine meets it.
  const grown = document.createElement('p');
  grown.style.height = '120px';
  fold.focus();
  document.querySelector('.thread-messages')?.append(grown);
  await expect.poll(() => inView(fold), { message: 'the line with focus' }).toBe(true);
  grown.remove();
  // The chat knows it is away from its foot: its down arrow shows.
  await expect.poll(() => document.querySelector<HTMLButtonElement>('.thread-bottom')?.disabled).toBe(false);

  await userEvent.keyboard('{Enter}');
  await expect.poll(() => foldOf(bo())?.textContent).toBe('Collapse replies');
  await expect.poll(() => inView(replyWith('Rev, PR 22 is yours to review.'))).toBe(true);
  expect(inView(foldOf(bo()))).toBe(true);
});

test('a reply said while the person was away says New after its time and nothing else, whatever its turn did', async () => {
  const day = morning();
  // Ann's word in Bo's thread worked on her own task, ran a step that failed, and was cut when its task was cancelled.
  day.turns = day.turns.map((t) =>
    t.id === 'ann2' ? { ...t, task: 2, status: 'stopped', end: 'cancelled' } : t,
  );
  const tool: Message = {
    id: 't-ann2',
    turn: 'ann2',
    time: at(10, 2),
    role: 'assistant',
    kind: 'tool',
    body: { tool: 'Bash', status: 'error' },
  };
  day.messages.splice(
    day.messages.findIndex((m) => m.id === 'w-ann2'),
    0,
    tool,
  );
  service(day, left());
  await render(<Chat />);
  const bo = () => mainWith('Shopping list screen');
  await expect.poll(() => foldOf(bo())?.textContent).toBe('3 new replies');
  (foldOf(bo()) as HTMLButtonElement).click();
  const line = () => replyWith('Nice. The shape is final.')?.querySelector('.name-line');
  await expect.poll(() => line()?.querySelector('.name-line-new')?.textContent).toBe('New');
  const pieces = [
    ...(line()?.querySelectorAll(
      '.name-line-meta > *, .name-line > :not(.name-line-name, .name-line-meta)',
    ) ?? []),
  ];
  expect(pieces.map((p) => (p.tagName === 'TIME' ? 'time' : p.textContent))).toEqual(['time', 'New']);
});

test('opened, a long thread shows every reply said while the person was away, not only its newest three', async () => {
  service(
    morning(),
    left().map((r) => (r.topic === 't:3' ? { ...r, seen_us: us(at(9, 35)) } : r)),
  );
  await render(<Chat />);
  const bo = () => mainWith('Shopping list screen');
  await expect.poll(() => foldOf(bo())?.textContent).toBe('4 new replies');
  await (foldOf(bo()) as HTMLButtonElement).click();
  await expect
    .poll(() => replyWith('Bo, the planner screen is yours.')?.querySelector('.name-line-new')?.textContent)
    .toBe('New');
  expect(document.body.textContent).not.toContain('earlier repl');
});

test('a reply arriving out of view shows 1 New; a press opens its folded thread at it, and the pill goes', async () => {
  const day = morning(['ann3']);
  // All seen, and Ann's thread folded by the person.
  const reads = left().map((r) => ({
    ...r,
    seen_us: us(at(10, 7)),
    folded: r.topic === 't:2' ? true : r.folded,
  }));
  const { puts } = service(day, reads);
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
  expect(pill()).toBeNull();

  const ann = turn('ann3', 'ann', 't:2', at(10, 8), { task: 2 });
  send({
    type: 'turn',
    chat: 'c',
    turn: 'ann3',
    status: 'done',
    agent: 'ann',
    created: ann.created,
    task: 2,
    topic: 't:2',
    position: ann.position,
  });
  send({ type: 'message', chat: 'c', message: words('w-ann3', 'ann3', ANN_NEW, at(10, 8)) });
  await expect.poll(() => pill()?.textContent).toBe('1 New');
  expect(pill()?.getAttribute('aria-label')).toBe('1 new message. Go to it');
  // Its thread's main message is near the top of the chat, above the view.
  // The arrow follows what the view shows, measured a frame after the pill is drawn.
  await expect.poll(arrow).toBe('up');
  // Ann's thread stays folded: a new reply never opens it.
  expect(foldOf(mainWith('Add up ingredients'))?.textContent).toBe('1 new reply');

  await (pill() as HTMLButtonElement).click();
  await expect.poll(() => foldOf(mainWith('Add up ingredients'))?.getAttribute('aria-expanded')).toBe('true');
  await expect.poll(() => replyWith(ANN_NEW)?.hasAttribute('data-search-hit')).toBe(true);
  await expect.poll(() => pill()).toBeNull();
  await written(() => sentFor(puts, 't:2')).toBeGreaterThanOrEqual(us(at(10, 8)));
});

test('three unseen blocks in three threads read 3 New; each press goes to the oldest, and the count drops', async () => {
  // Unseen: the answer to side ask 0 (09:41:30) and to side ask 4 (09:45:30), each far from the other, and Mira's
  // word in Bo's folded thread (10:03). Mira's word at the foot is in view, so seen as the chat opens.
  const reads = left().map((r) => {
    if (r.topic === 't:2') return { ...r, seen_us: us(at(10, 8)) };
    if (r.topic === 't:3') return { ...r, seen_us: us(at(10, 2)) };
    if (r.topic === 'm:u-f0' || r.topic === 'm:u-f4') return { ...r, seen_us: us(at(9, 40)) };
    return r;
  });
  const { puts } = service(morning(), reads);
  await render(<Chat />);
  await expect.poll(() => pill()?.textContent).toBe('3 New');
  expect(pill()?.getAttribute('aria-label')).toBe('3 new messages. Go to the oldest');
  // The arrow follows what the view shows, measured a frame after the pill is drawn.
  await expect.poll(arrow).toBe('up');

  await (pill() as HTMLButtonElement).click();
  await expect.poll(() => replyWith('Made-up answer 0')?.hasAttribute('data-search-hit')).toBe(true);
  await expect.poll(() => pill()?.textContent).toBe('2 New');
  // Side ask 4's answer is below what the view shows once it has moved to side ask 0.
  await expect.poll(arrow).toBe('down');

  await (pill() as HTMLButtonElement).click();
  await expect.poll(() => replyWith('Made-up answer 4')?.hasAttribute('data-search-hit')).toBe(true);
  await expect.poll(() => pill()?.textContent).toBe('1 New');
  await expect.poll(arrow).toBe('up');

  // The last is in Bo's folded thread: the press opens it.
  await (pill() as HTMLButtonElement).click();
  await expect
    .poll(() => foldOf(mainWith('Shopping list screen'))?.getAttribute('aria-expanded'))
    .toBe('true');
  await expect
    .poll(() => replyWith('Rev, PR 22 is yours to review.')?.hasAttribute('data-search-hit'))
    .toBe(true);
  await expect.poll(() => pill()).toBeNull();
  await written(() => sentFor(puts, 't:3')).toBeGreaterThanOrEqual(us(at(10, 3)));
});

test('an unseen main message counts, and a press goes to it and marks it seen', async () => {
  // The window in the back: Mira's word at the foot shows but isn't seen.
  vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  const reads = left().map((r) => {
    if (r.topic === 't:2') return { ...r, seen_us: us(at(10, 8)) };
    if (r.topic === 't:3') return { ...r, seen_us: us(at(10, 3)) };
    return r;
  });
  const { puts } = service(morning(), reads);
  await render(<Chat />);
  await expect.poll(() => pill()?.textContent).toBe('1 New');
  await (pill() as HTMLButtonElement).click();
  await expect
    .poll(() => mainWith('Bo’s screen lists ingredients')?.hasAttribute('data-search-hit'))
    .toBe(true);
  await expect.poll(() => pill()).toBeNull();
  await written(() => sentFor(puts, '')).toBeGreaterThanOrEqual(us(at(10, 5)));
});

test('above 99 the pill reads 99+ New', async () => {
  vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  const day = morning();
  // A hundred more answers in side ask 9's thread from 10:20, each a second apart, none seen.
  for (let i = 0; i < 100; i++) {
    const id = `more${i}`;
    const time = at(10, 20 + Math.floor(i / 60), i % 60);
    day.turns.push(turn(id, 'mira', 'm:u-f9', time));
    day.messages.push(words(`w-${id}`, id, `Made-up more ${i}`, time));
  }
  service(day, left());
  await render(<Chat />);
  await expect.poll(() => pill()?.textContent).toBe('99+ New');
});

test('a reply arriving in view is seen at once and never shows a pill', async () => {
  // Mira is still at work on her word at the foot: its thread is open.
  const day = morning(['ann3'], ['wake']);
  const { puts } = service(
    day,
    left().map((r) => ({ ...r, seen_us: us(at(10, 7)) })),
  );
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
  // Any pill drawn at all, for however short a time.
  let shown = 0;
  const watch = new MutationObserver(() => {
    if (pill()) shown += 1;
  });
  watch.observe(document.body, { childList: true, subtree: true, characterData: true });
  // Bo answers under Mira's word, at the foot of the chat, where the person is.
  send({
    type: 'turn',
    chat: 'c',
    turn: 'bo3',
    status: 'done',
    agent: 'bo',
    created: at(10, 9),
    topic: 'b:wake',
    position: 90,
  });
  send({
    type: 'message',
    chat: 'c',
    message: words('w-bo3', 'bo3', 'Recipe order is on purpose.', at(10, 9)),
  });
  await expect.element(page.getByText('Recipe order is on purpose.')).toBeVisible();
  await written(() => sentFor(puts, 'b:wake')).toBeGreaterThanOrEqual(us(at(10, 9)));
  watch.disconnect();
  expect(shown).toBe(0);
});

test('nothing is seen while the window is in the back; brought to the front, what shows is', async () => {
  vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  const { puts } = service(morning(), left());
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
  // The pill counts what is new, wherever it is, while nothing is seen: Mira's word at the foot, Bo's three folded
  // replies and Ann's newest.
  await expect.poll(() => pill()?.textContent).toBe('5 New');
  await new Promise((r) => setTimeout(r, 1200));
  expect(sentFor(puts, '')).toBe(0);

  vi.spyOn(document, 'hasFocus').mockReturnValue(true);
  window.dispatchEvent(new FocusEvent('focus'));
  await written(() => sentFor(puts, '')).toBeGreaterThanOrEqual(us(at(10, 5)));
});

test('a window opened in the back and first brought to the front shows what came meanwhile as New', async () => {
  // As Gathr opened at login, hidden: the window has never been in front, so it has never left it either.
  vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  service(
    morning(),
    left().map((r) => ({ ...r, seen_us: us(at(10, 9)) })),
  );
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
  expect(document.querySelector('.new-line')).toBeNull();
  const now = new Date().toISOString();
  send({
    type: 'turn',
    chat: 'c',
    turn: 'late',
    status: 'done',
    agent: 'mira',
    created: now,
    topic: 'b:late',
    position: 91,
  });
  send({ type: 'message', chat: 'c', message: words('w-late', 'late', 'A made-up late word.', now) });
  await expect.element(page.getByText('A made-up late word.')).toBeVisible();
  expect(document.querySelector('.new-line')).toBeNull();

  vi.spyOn(document, 'hasFocus').mockReturnValue(true);
  window.dispatchEvent(new FocusEvent('focus'));
  await expect
    .poll(() => document.querySelector('.new-line')?.nextElementSibling?.textContent)
    .toContain('A made-up late word.');
});

test('marks from another window come live: the blue count and the pill follow them', async () => {
  service(morning(), left());
  await render(<Chat />);
  // Bo's three folded replies and Ann's newest; Mira's word at the foot is seen as it shows.
  await expect.poll(() => pill()?.textContent).toBe('4 New');
  await expect.poll(() => foldOf(mainWith('Shopping list screen'))?.textContent).toBe('3 new replies');
  const seen: Read[] = [
    { topic: 't:2', seen_us: us(at(10, 8)), folded: null },
    { topic: 't:3', seen_us: us(at(10, 3)), folded: true },
  ];
  send({ type: 'reads', chat: 'c', reads: seen });
  await expect.poll(() => pill()).toBeNull();
  await expect.poll(() => foldOf(mainWith('Shopping list screen'))?.textContent).toBe('4 replies');
});

test('a handled thread all seen opens folded, one with a reply not seen opens open, and neither moves while open', async () => {
  const day = morning();
  // Tam's reply at 09:52 isn't seen; Ann's thread is all seen.
  const reads = left([{ topic: 't:2', seen_us: us(at(10, 10)), folded: null }]).map((r) =>
    r.topic === 't:1' ? { ...r, seen_us: us(at(9, 40)) } : r,
  );
  const { kept } = service(day, reads);
  const screen = await render(<Chat />);
  await expect
    .poll(() => foldOf(mainWith('Made-up side ask 3'))?.getAttribute('aria-expanded'))
    .toBe('false');
  expect(foldOf(mainWith('Plan the made-up shopping list'))?.getAttribute('aria-expanded')).toBe('false');
  expect(foldOf(mainWith('Tidy units in old recipes'))?.getAttribute('aria-expanded')).toBe('true');
  expect(foldOf(mainWith('Add up ingredients'))?.getAttribute('aria-expanded')).toBe('true');

  // Ann's task is done while the chat is open: her thread stays open. Her face's mark says the task came.
  const annMark = () =>
    document.querySelector('.summary-button .face[data-agent="ann"] .mark')?.getAttribute('data-state') ??
    '-';
  const owed = annMark();
  day.tasks[1] = { ...day.tasks[1], state: 'done' };
  send({ type: 'tasks', chat: 'c', tasks: day.tasks, waits: [], asks: [] });
  await expect.poll(annMark).not.toBe(owed);
  expect(foldOf(mainWith('Add up ingredients'))?.getAttribute('aria-expanded')).toBe('true');

  // Opened again, as after a reload: handled and seen, it starts folded.
  expect(kept.get('t:2')?.seen_us).toBeGreaterThanOrEqual(us(at(10, 8)));
  await screen.rerender(<Chat n={1} />);
  await expect
    .poll(() => foldOf(mainWith('Add up ingredients'))?.getAttribute('aria-expanded'))
    .toBe('false');
});

test('the person’s own fold or open is kept, wins over the rule, and holds after a reload', async () => {
  const { puts } = service(
    morning(),
    left().map((r) => (r.topic === 'm:u-f3' ? { ...r, folded: false } : r)),
  );
  const screen = await render(<Chat />);
  const side = () => foldOf(mainWith('Made-up side ask 3'));
  await expect.poll(() => side()?.getAttribute('aria-expanded')).toBe('true');
  expect(foldOf(mainWith('Made-up side ask 4'))?.getAttribute('aria-expanded')).toBe('false');

  (side() as HTMLButtonElement).click();
  await written(() => puts.flat().find((r) => r.topic === 'm:u-f3' && r.folded === true)).toBeTruthy();
  await screen.rerender(<Chat n={1} />);
  await expect.poll(() => side()?.getAttribute('aria-expanded')).toBe('false');
  expect(foldOf(mainWith('Shopping list screen'))?.getAttribute('aria-expanded')).toBe('false');
});

test('a chat with no marks opens with nothing new and its handled threads folded, and keeps that', async () => {
  const { puts } = service(morning(), []);
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
  await written(() => sentFor(puts, '')).toBeGreaterThanOrEqual(us(at(10, 8)));
  expect(document.querySelector('.new-line')).toBeNull();
  expect(document.querySelector('.name-line-new')).toBeNull();
  expect(pill()).toBeNull();
  expect(foldOf(mainWith('Made-up side ask 3'))?.getAttribute('aria-expanded')).toBe('false');
  expect(foldOf(mainWith('Tidy units in old recipes'))?.getAttribute('aria-expanded')).toBe('false');
  // Every thread there is kept as seen up to now.
  for (const topic of ['t:1', 't:2', 't:3', 'm:u-ask', 'b:wake'])
    expect(sentFor(puts, topic), topic).toBeGreaterThanOrEqual(us(at(10, 8)));
});

test('seeing messages as the chat scrolls draws no message again', async () => {
  const { puts } = service(morning(), left());
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
  const viewport = document.querySelector('.thread-viewport') as HTMLElement;
  await written(() => sentFor(puts, '')).toBeGreaterThan(0);
  const stop = watchRenders();
  const writes = puts.length;
  for (let top = viewport.scrollTop; top > 0; top -= 120) {
    viewport.scrollTop = top;
    await new Promise((r) => requestAnimationFrame(r));
  }
  viewport.scrollTop = 0;
  // Ann's thread, at the top, is seen: its mark goes out.
  await written(() => sentFor(puts, 't:2')).toBeGreaterThanOrEqual(us(at(10, 8)));
  const drawn = stop();
  expect(drawn).toEqual({});
  // Many marks, few writes: at most one a second.
  expect(puts.length - writes).toBeLessThanOrEqual(2);
});
