// Where the person left off (0077), in the chat as the window draws it, the service at the edge: fetch answers as it
// would, keeping the marks it is sent, and the live socket is a stand-in. The New line, the blue counts, each reply's
// New, the line at the foot, the folds, and what a chat with no marks opens as. Pointer and keyboard.
import './renderWatch';
import { useState } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, History, Member, Message, Read, Task, Turn } from '../api';
import type { LiveEvent } from '../live';
import { ChatView } from './ChatView';
import { watchRenders } from './renderWatch';
import { clock } from './team';
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
const footLine = () => document.querySelector<HTMLButtonElement>('.foot-line');
/** The newest mark sent for a thread, in µs. */
const sentFor = (puts: Read[][], topic: string) =>
  Math.max(0, ...puts.flat().flatMap((r) => (r.topic === topic && r.seen_us ? [r.seen_us] : [])));
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
  service(morning(), left());
  await render(<Chat />);
  const bo = () => mainWith('Shopping list screen');
  await expect.poll(() => foldOf(bo())?.textContent).toBe('3 new replies');
  const fold = foldOf(bo()) as HTMLButtonElement;
  expect(fold.getAttribute('aria-expanded')).toBe('false');
  expect(getComputedStyle(fold).color).toBe(attention());

  fold.focus();
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
  await page.getByRole('button', { name: 'Show 1 earlier reply' }).click();
  await expect.poll(() => !!replyWith('Bo, the planner screen is yours.')).toBe(true);
  expect(replyWith('Bo, the planner screen is yours.')?.querySelector('.name-line-new')).toBeNull();
  // Seen once opened: folded again, it counts them all, plainly.
  await (foldOf(bo()) as HTMLButtonElement).click();
  await expect.poll(() => foldOf(bo())?.textContent).toBe('4 replies');
  expect(getComputedStyle(foldOf(bo()) as HTMLElement).color).not.toBe(attention());
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

test('a reply arriving out of view is named at the foot; a click goes there, opens its thread, and the line goes', async () => {
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
  expect(footLine()).toBeNull();

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
  await expect.poll(() => footLine()?.textContent).toContain(`Ann replied: ${ANN_NEW}`);
  expect(footLine()?.textContent).toContain(clock(at(10, 8)));
  // Ann's thread stays folded: a new reply never opens it.
  expect(foldOf(mainWith('Add up ingredients'))?.textContent).toBe('1 new reply');

  await (footLine() as HTMLButtonElement).click();
  await expect.poll(() => foldOf(mainWith('Add up ingredients'))?.getAttribute('aria-expanded')).toBe('true');
  await expect.poll(() => replyWith(ANN_NEW)?.hasAttribute('data-search-hit')).toBe(true);
  await expect.poll(() => footLine()).toBeNull();
  await expect.poll(() => sentFor(puts, 't:2')).toBeGreaterThanOrEqual(us(at(10, 8)));
});

test('a reply arriving in view is seen at once and named nowhere', async () => {
  // Mira is still at work on her word at the foot: its thread is open.
  const day = morning(['ann3'], ['wake']);
  const { puts } = service(
    day,
    left().map((r) => ({ ...r, seen_us: us(at(10, 7)) })),
  );
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
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
  await expect.poll(() => sentFor(puts, 'b:wake')).toBeGreaterThanOrEqual(us(at(10, 9)));
  expect(footLine()).toBeNull();
});

test('nothing is seen while the window is in the back; brought to the front, what shows is', async () => {
  vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  const { puts } = service(morning(), left());
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
  // The line at the foot names what is new, wherever it is, while nothing is seen.
  await expect.poll(() => footLine()?.textContent).toContain('Ann replied');
  await new Promise((r) => setTimeout(r, 1200));
  expect(sentFor(puts, '')).toBe(0);

  vi.spyOn(document, 'hasFocus').mockReturnValue(true);
  window.dispatchEvent(new FocusEvent('focus'));
  await expect.poll(() => sentFor(puts, '')).toBeGreaterThanOrEqual(us(at(10, 5)));
});

test('marks from another window come live: the blue count and the line at the foot follow them', async () => {
  service(morning(), left());
  await render(<Chat />);
  await expect.poll(() => footLine()?.textContent).toContain('Ann replied');
  await expect.poll(() => foldOf(mainWith('Shopping list screen'))?.textContent).toBe('3 new replies');
  const seen: Read[] = [
    { topic: 't:2', seen_us: us(at(10, 8)), folded: null },
    { topic: 't:3', seen_us: us(at(10, 3)), folded: true },
  ];
  send({ type: 'reads', chat: 'c', reads: seen });
  await expect.poll(() => footLine()).toBeNull();
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

  // Ann's task is done while the chat is open: her thread stays open.
  day.tasks[1] = { ...day.tasks[1], state: 'done' };
  send({ type: 'tasks', chat: 'c', tasks: day.tasks, waits: [], asks: [] });
  await expect
    .poll(() => mainWith('Add up ingredients')?.querySelector('.main-state-word')?.textContent)
    .toBe('Done');
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
  await expect.poll(() => puts.flat().find((r) => r.topic === 'm:u-f3' && r.folded === true)).toBeTruthy();
  await screen.rerender(<Chat n={1} />);
  await expect.poll(() => side()?.getAttribute('aria-expanded')).toBe('false');
  expect(foldOf(mainWith('Shopping list screen'))?.getAttribute('aria-expanded')).toBe('false');
});

test('a chat with no marks opens with nothing new and its handled threads folded, and keeps that', async () => {
  const { puts } = service(morning(), []);
  await render(<Chat />);
  await expect.element(page.getByText('Bo’s screen lists ingredients in recipe order.')).toBeVisible();
  await expect.poll(() => sentFor(puts, '')).toBeGreaterThanOrEqual(us(at(10, 8)));
  expect(document.querySelector('.new-line')).toBeNull();
  expect(document.querySelector('.name-line-new')).toBeNull();
  expect(footLine()).toBeNull();
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
  await expect.poll(() => sentFor(puts, '')).toBeGreaterThan(0);
  const stop = watchRenders();
  const writes = puts.length;
  for (let top = viewport.scrollTop; top > 0; top -= 120) {
    viewport.scrollTop = top;
    await new Promise((r) => requestAnimationFrame(r));
  }
  viewport.scrollTop = 0;
  // Ann's thread, at the top, is seen: its mark goes out.
  await expect.poll(() => sentFor(puts, 't:2')).toBeGreaterThanOrEqual(us(at(10, 8)));
  const drawn = stop();
  expect(drawn).toEqual({});
  // Many marks, few writes: at most one a second.
  expect(puts.length - writes).toBeLessThanOrEqual(2);
});
