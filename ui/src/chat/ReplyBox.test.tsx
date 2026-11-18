// Replying in a thread (0076): Reply on a message, a task's message or a thread's line opens the one message box at
// the thread's foot with focus in it; Escape and the cross close it and focus goes back; one box at a time; each thread
// keeps its own draft; Enter sends the reply with what it answers and to whom; in a waiting question's thread the box
// talks it over and the question stays open; the main box still sends a new main message; in a chat of one agent a
// reply to an older message makes a thread under it. Made-up names and words only.
import { useRef, useState } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message, Task, Turn } from '../api';
import type { Picked } from '../components/mention';
import { Question } from '../tasks/Question';
import { ChatView } from './ChatView';
import { toThread } from './messages';
import { type ChatState, empty } from './state';

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
const agents = [made('a', 'Made-up Lead'), made('b', 'Ann')];
const turn = (id: string, agent: string, position: number, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  position,
  created: `2026-11-08T10:0${position}:00Z`,
  ...more,
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
const at = (minute: number) => Date.parse(`2026-11-08T10:${String(minute).padStart(2, '0')}:00Z`) * 1000;
const tasks = [
  {
    number: 1,
    chat: 'c',
    giver: 'a',
    owner: 'b',
    given_in: 't1',
    kind: 'work',
    what: 'Made-up walk map',
    why: 'Made-up reason',
    state: 'running',
    reason: null,
    after: [],
    created_us: at(3),
    ask: 'p1',
    topic: 't:1',
  },
  {
    number: 2,
    chat: 'c',
    giver: 'a',
    owner: null,
    given_in: 't3',
    kind: 'person',
    what: 'Made-up question: the long way?',
    state: 'ready',
    reason: null,
    after: [],
    created_us: at(8),
    form: 'choice',
    choices: [{ label: 'Made-up yes' }, { label: 'Made-up no' }],
    ask: 'p1',
    topic: 't:2',
  },
] as unknown as Task[];

/** A team chat: the person's ask, the lead's answer under it, a task handed to Ann with her words, a question. */
function history() {
  return {
    chat: 'c',
    members,
    approvals: [],
    turns: [
      turn('t1', 'a', 1, { topic: 'm:p1' }),
      turn('t2', 'b', 4, { task: 1, topic: 't:1' }),
      turn('t3', 'a', 7, { topic: 'm:p1' }),
    ],
    messages: [
      said('p1', 't1', 1, 'Plan the made-up walk', { role: 'user' }),
      said('l1', 't1', 2, 'Made-up plan words'),
      said('b1', 't2', 5, 'Made-up work words'),
      said('q1', 't3', 8, 'Made-up question words'),
    ],
  };
}

/** A service that answers the chat; what was sent, each body as JSON, by path. */
function service() {
  const sent: { path: string; body: unknown }[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.endsWith('/ui/events')) return new Response('{}');
    if (init?.method === 'POST') {
      sent.push({ path, body: JSON.parse(String(init.body)) });
      if (path.endsWith('/answer')) return new Response(JSON.stringify({ answered: [], refused: [] }));
      return new Response(JSON.stringify({ turn: 'new', status: 'queued' }));
    }
    if (path.endsWith('/tasks'))
      return new Response(
        JSON.stringify({
          tasks,
          waits: [],
          asks: [
            { id: 'p1', title: 'Plan the made-up walk', size: 'repair', by: 'a', held: false, open: true },
          ],
          plans: [],
          caught: [],
        }),
      );
    if (path.endsWith('/c')) return new Response(JSON.stringify(history()));
    return new Response('{}');
  });
  return sent;
}

afterEach(() => vi.unstubAllGlobals());

/** The chat as the app keeps it: the main draft, and each thread's draft beside it. */
function Open() {
  const [draft, setDraft] = useState<{ text: string; picks: Picked[] }>({ text: '', picks: [] });
  const threads = useRef(new Map<string, { text: string; picks: Picked[] }>());
  const [, redraw] = useState(0);
  const answers = useRef({
    drafts: {},
    onDraft: () => {},
    onSend: async (task: Task, d: unknown) => {
      await fetch('/api/person-tasks/answer', {
        method: 'POST',
        body: JSON.stringify({ task: task.number, draft: d }),
      });
      return null;
    },
  });
  return (
    <ChatView
      chat={{ id: 'c', title: 'Made-up chat' }}
      agents={agents}
      draft={draft.text}
      picks={draft.picks}
      onDraft={(text, picks = []) => setDraft({ text, picks })}
      replyDraft={(topic) => threads.current.get(topic)}
      onReplyDraft={(topic, text, picks) => {
        threads.current.set(topic, { text, picks });
        redraw((n) => n + 1);
      }}
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
      answers={answers.current}
    />
  );
}

/** The message whose bubble holds `words`, by its outermost block. */
const message = (words: string) =>
  page.getByText(words, { exact: true }).element().closest('.said') as HTMLElement;
/** Reply on the corner row of the message holding `words`. */
async function replyOn(words: string) {
  const row = message(words);
  await userEvent.hover(row);
  const button = row.querySelector<HTMLElement>('.message-actions [aria-label="Reply"]');
  expect(button, `Reply on "${words}"`).not.toBeNull();
  await userEvent.click(button as HTMLElement);
  return button as HTMLElement;
}
const box = () => page.getByRole('textbox', { name: 'Reply' });
const main = () => page.getByRole('textbox', { name: 'Message' });
/** The items of the chat, in reading order: each one's text. */
const order = () => [...document.querySelectorAll('.thread-messages > *')];

test('Reply opens the box at the thread’s foot with focus in it; Escape and the cross close it, focus goes back', async () => {
  service();
  await render(<Open />);
  await expect.element(page.getByText('Made-up work words')).toBeVisible();
  const reply = await replyOn('Made-up work words');
  await expect.element(box()).toHaveFocus();
  // The task's thread: its main message, its fold line, Ann's words, then the box, in reading order.
  const items = order();
  const words = items.findIndex((e) => e.textContent?.includes('Made-up work words'));
  const foot = items.findIndex((e) => e.classList.contains('thread-box'));
  expect(words).toBeGreaterThan(-1);
  expect(foot).toBe(words + 1);
  await userEvent.keyboard('{Escape}');
  await expect.element(box()).not.toBeInTheDocument();
  expect(document.activeElement).toBe(reply);

  await userEvent.click(reply);
  await expect.element(box()).toHaveFocus();
  await page.getByRole('button', { name: 'Close' }).click();
  await expect.element(box()).not.toBeInTheDocument();
  expect(document.activeElement).toBe(reply);
});

test('one box at a time, each thread keeps its own draft, and the main box still sends a main message', async () => {
  const sent = service();
  await render(<Open />);
  await expect.element(page.getByText('Made-up plan words')).toBeVisible();
  await replyOn('Made-up plan words');
  await userEvent.keyboard('Made-up first draft');
  await replyOn('Made-up work words');
  await expect.poll(() => document.querySelectorAll('.thread-box').length).toBe(1);
  await expect.element(box()).toHaveValue('');
  await userEvent.keyboard('Made-up second draft');
  await replyOn('Made-up plan words');
  await expect.element(box()).toHaveValue('Made-up first draft');

  await main().click();
  await userEvent.keyboard('Made-up new ask{Enter}');
  await expect.poll(() => sent.map((s) => s.body)).toEqual([{ text: 'Made-up new ask', agent: 'a' }]);
  await expect.element(box()).toHaveValue('Made-up first draft');
});

test('Enter sends the reply with the message it answers, to its author; a task’s message, to its owner', async () => {
  const sent = service();
  await render(<Open />);
  await expect.element(page.getByText('Made-up work words')).toBeVisible();
  await replyOn('Made-up work words');
  await userEvent.keyboard('Made-up reply to Ann');
  await expect.element(page.getByRole('status').filter({ hasText: 'To Ann' })).toBeVisible();
  await userEvent.keyboard('{Enter}');
  await expect.element(box()).not.toBeInTheDocument();

  await replyOn('Made-up walk map');
  await userEvent.keyboard('Made-up words on the task{Enter}');
  await expect
    .poll(() => sent.map((s) => s.body))
    .toEqual([
      { text: 'Made-up reply to Ann', agent: 'b', reply_to: 'b1' },
      { text: 'Made-up words on the task', agent: 'b', reply_task: 1 },
    ]);
});

test('Reply on a thread’s line answers its main message: the person’s ask goes to the lead', async () => {
  const sent = service();
  await render(<Open />);
  await expect.element(page.getByText('Plan the made-up walk')).toBeVisible();
  const line = page.getByRole('button', { name: 'Reply', exact: true }).first();
  await line.click();
  await expect.element(box()).toHaveFocus();
  await userEvent.keyboard('Made-up words to the lead{Enter}');
  await expect
    .poll(() => sent.map((s) => s.body))
    .toEqual([{ text: 'Made-up words to the lead', agent: 'a', reply_to: 'p1' }]);
});

test('in a waiting question’s thread the box talks it over: a Discuss, and the question stays open', async () => {
  const sent = service();
  await render(<Open />);
  await expect.element(page.getByText('Made-up question words')).toBeVisible();
  await replyOn('Made-up question words');
  await userEvent.keyboard('Made-up why the long way{Enter}');
  await expect.poll(() => sent.map((s) => s.path)).toEqual(['/api/person-tasks/answer']);
  expect(sent[0].body).toEqual({
    task: 2,
    draft: { values: [], notes: '', discuss: true, message: 'Made-up why the long way' },
  });
  // Still open in its bubble: its choices are there to pick.
  await expect.element(page.getByRole('button', { name: /^Made-up yes/ })).toBeVisible();
});

test('the chat’s question block has no Discuss switch; the Tasks view’s does', async () => {
  service();
  await render(<Open />);
  await expect.element(page.getByText('Made-up question: the long way?')).toBeVisible();
  await expect.element(page.getByRole('switch', { name: 'Discuss' })).not.toBeInTheDocument();
  const question = { ...tasks[1], asker: 'a' } as unknown as Parameters<typeof Question>[0]['task'];
  await render(
    <Question
      task={question}
      draft={{ values: [], notes: '' }}
      onDraft={() => {}}
      name={() => 'Made-up Lead'}
      onSend={async () => null}
      onReference={() => {}}
    />,
  );
  await expect.element(page.getByRole('switch', { name: 'Discuss' })).toBeVisible();
});

/** A chat of one agent: the person's two messages and the agent's two answers, then a reply to the first answer. */
function solo(): ChatState {
  return {
    ...empty,
    turns: {
      s1: turn('s1', 'b', 1),
      s2: turn('s2', 'b', 2),
      s3: turn('s3', 'b', 3),
    },
    messages: [
      said('y1', 's1', 10, 'Made-up first ask', { role: 'user' }),
      said('n1', 's1', 12, 'Made-up first answer'),
      said('y2', 's2', 13, 'Made-up second ask', { role: 'user' }),
      said('n2', 's2', 20, 'Made-up second answer'),
      said('y3', 's3', 25, 'Made-up reply to the first answer', { role: 'user', reply_to: 'n1' }),
      said('n3', 's3', 27, 'Made-up answer in the thread'),
    ],
  };
}

test('in a chat of one agent a reply to an older message makes a thread under it', () => {
  // As the chat draws it after the reply was sent: the thread wasn't there when it opened, so it is open.
  const view = { open: new Map(), start: new Set<string>(), earlier: new Set<string>() };
  const timeline = toThread(solo(), [made('b', 'Ann')], [], new Map(), view);
  const read = timeline.items.map((m) => {
    const own = m.metadata?.custom as { fold?: { count: number }; reply?: boolean } | undefined;
    if (own?.fold) return `fold ${own.fold.count}`;
    const parts = typeof m.content === 'string' ? [{ type: 'text', text: m.content }] : m.content;
    const text = parts.find((c) => c.type === 'text');
    const words = text && 'text' in text ? text.text : '';
    return own?.reply ? `  ${words}` : words;
  });
  expect(read).toEqual([
    'Made-up first ask',
    'Made-up first answer',
    'fold 2',
    '  Made-up reply to the first answer',
    '  Made-up answer in the thread',
    'Made-up second ask',
    'Made-up second answer',
  ]);
  // The map marks the main line: the person's asks, never their reply in a thread.
  const ticked = timeline.items
    .filter((m) => (m.metadata?.custom as { tick?: string } | undefined)?.tick)
    .map((m) => m.id);
  expect(ticked).not.toContain('y3');
  expect(ticked).toContain('y1');
});
