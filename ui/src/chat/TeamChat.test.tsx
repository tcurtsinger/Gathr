// The chat read as a team chat (0072): every message beside its author's face, talk between agents as a thread
// under the message it answers, an @Name for whom it is to, thumbs-ups, and a turn's steps on its name line.
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { page, server, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Message, ToolBody, Turn } from '../api';
import { useFolds } from './folds';
import { toThread } from './messages';
import { apply, type ChatState, empty } from './state';
import { type Actions, Thread } from './Thread';

const members = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' as const },
  { agent: 'b', name: 'Made-up Member', role: 'member' as const },
  { agent: 'c', name: 'Made-up Reviewer', role: 'reviewer' as const },
];
const agents: Agent[] = members.map((m) => ({
  id: m.agent,
  name: m.name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto',
  face_seed: 'Made-up',
  gender: null,
  skin: null,
  background: null,
}));
const turn = (id: string, agent: string, position: number, more: Partial<Turn> = {}): Turn => ({
  id,
  agent,
  status: 'done',
  why: null,
  ref: id,
  position,
  ...more,
});
let minute = 0;
const said = (id: string, turnId: string, body: string, more: Partial<Message> = {}): Message => ({
  id,
  turn: turnId,
  time: `2026-11-08T09:${String(minute++ % 60).padStart(2, '0')}:00Z`,
  role: 'assistant',
  kind: 'text',
  body,
  ...more,
});
/** What Gathr told the receiver in its turn: a copy of the message it carries. */
const copy = (id: string, turnId: string, of: string) =>
  said(id, turnId, 'Made-up words, carried', { role: 'gathr', copy_of: of });
const tool = (id: string, turnId: string, name: string, more: Partial<ToolBody> = {}): Message =>
  said(id, turnId, '', { kind: 'tool', body: { tool: name, status: 'ok', ...more } });

function View({ state }: { state: ChatState }) {
  const cache = useRef(new Map());
  // Every thread starts open here; a press on its fold line folds it, as in the chat.
  const threads = useFolds();
  const none = () => {};
  const actions: Actions = {
    fold: threads.fold,
    earlier: threads.earlier,
    openTask: none,
    openAgent: none,
    diagnostics: none,
    reference: none,
    signIn: none,
    retry: none,
    resend: none,
    carryOn: none,
    approve: none,
  };
  return (
    <div style={{ height: 900, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, [], cache.current, { ...threads.folds, start: new Set() }).items}
        running={false}
        actions={actions}
        empty={false}
      />
    </div>
  );
}

/** The drawn messages, top to bottom, by their words. */
const order = (...words: string[]) =>
  words
    .map((w) => page.getByText(w, { exact: true }).element())
    .every((el, i, all) =>
      i === 0 ? true : all[i - 1].compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING,
    );

/**
 * The member works and asks the lead; the person writes before the lead answers; the lead's answer, from a turn
 * its message woke, belongs under the question, not after the person's words.
 */
function talk(): ChatState {
  return {
    ...empty,
    members,
    // What each turn is about, as the service names it: the member's turn nothing ties is its own main message, and the
    // turn its message opened sits with it.
    turns: {
      work: turn('work', 'b', 0, { topic: 'b:work' }),
      you: turn('you', 'a', 1, { topic: 'm:u' }),
      told: turn('told', 'a', 2, { answers: 'work', topic: 'b:work' }),
    },
    messages: [
      said('found', 'work', 'Made-up finding'),
      said('q', 'work', 'Made-up question', { to: 'a' }),
      said('u', 'you', 'Made-up words of the person', { role: 'user' }),
      copy('cq', 'told', 'q'),
      tool('t', 'told', 'mcp__gathr__message'),
      said('r', 'told', 'Made-up answer', { to: 'b' }),
    ],
  };
}

test('a reply between agents reads in the thread of what it answers, and the thread folds', async () => {
  const screen = await render(<View state={talk()} />);
  await expect.element(screen.getByText('Made-up answer')).toBeVisible();
  // Under the member's message and the question, before what the person wrote in between.
  expect(order('Made-up finding', 'Made-up question', 'Made-up answer', 'Made-up words of the person')).toBe(
    true,
  );
  const replies = [...document.querySelectorAll('.said[data-reply]')].map((r) => r.textContent).join(' ');
  expect(replies).toContain('Made-up question');
  expect(replies).toContain('Made-up answer');
  // The turn that only answered has no message of its own: its step is on the answer's name line.
  expect(document.querySelectorAll('.said')).toHaveLength(4);

  await screen.getByRole('button', { name: 'Collapse replies' }).click();
  await expect.element(screen.getByText('Made-up answer')).not.toBeInTheDocument();
  const line = screen.getByRole('button', { name: '2 replies' });
  expect(line.element().getAttribute('aria-expanded')).toBe('false');
  await line.click();
  await expect.element(screen.getByText('Made-up answer')).toBeVisible();
});

test('a message to an agent starts with its @Name; one that quotes what it answers has none', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: {
      work: turn('work', 'b', 0),
      told: turn('told', 'a', 1, { answers: 'work' }),
      more: turn('more', 'a', 2, { answers: 'work' }),
    },
    messages: [
      said('first', 'work', 'Made-up first question', { to: 'a' }),
      said('second', 'work', 'Made-up second question', { to: 'a' }),
      copy('c1', 'told', 'first'),
      said('r1', 'told', 'Made-up answer to the first', { to: 'b' }),
      copy('c2', 'more', 'second'),
      said('r2', 'more', 'Made-up answer to the second', { to: 'b' }),
      // Written with its @Name already: named once.
      said('x', 'rev', '@Made-up Member made-up heads-up', { to: 'b' }),
    ],
  };
  state.turns.rev = turn('rev', 'c', 3);
  const screen = await render(<View state={state} />);
  const bubble = (words: string) =>
    [...document.querySelectorAll('.bubble')].find(
      (b) => b.querySelector('.markdown')?.textContent === words,
    ) as HTMLElement;
  await expect.element(screen.getByText('Made-up answer to the first')).toBeVisible();
  expect(bubble('Made-up first question').textContent).toBe('@Made-up LeadMade-up first question');
  expect(bubble('Made-up second question').querySelector('.mention')?.textContent).toBe('@Made-up Lead');
  // The second question came between: the answer quotes the first, plainly, and names nobody.
  const quoted = bubble('Made-up answer to the first');
  expect(quoted.querySelector('.bubble-quote')?.textContent).toBe('Made-up first question');
  expect(quoted.querySelector('.mention')).toBeNull();
  expect(bubble('Made-up answer to the second').querySelector('.bubble-quote')?.textContent).toBe(
    'Made-up second question',
  );
  const named = bubble('made-up heads-up');
  expect(named.querySelectorAll('.mention')).toHaveLength(1);
  expect(named.textContent).toBe('@Made-up Membermade-up heads-up');
});

test('a thumbs-up is the emoji and a count; who gave it shows on hover or focus, and one given live counts', async () => {
  const given = (message: string, agent: string, n: number) => ({
    message,
    agent,
    time: `2026-11-08T09:0${n}:00Z`,
  });
  const before: ChatState = {
    ...talk(),
    reactions: [given('found', 'gone', 0), given('found', 'c', 1), given('q', 'a', 2), given('u', 'c', 3)],
  };
  const screen = await render(<View state={before} />);
  // The member's finding is a main message: its thumbs-up is in the row under its bubble, with its fold line (0076).
  const said = screen.getByText('Made-up finding').element().closest('.said') as HTMLElement;
  const finding = said.querySelector('.said-foot') as HTMLElement;
  const mark = page.elementLocator(finding).getByRole('button', { name: 'Thumbs-up from Made-up Reviewer' });
  await expect.element(mark).toBeVisible();
  // The emoji and the count; the names are hidden until asked for.
  expect([...mark.element().children].slice(0, 2).map((c) => c.textContent)).toEqual(['👍', '1']);
  // For the member's words and its question; none for the person's own.
  expect(document.querySelectorAll('.react')).toHaveLength(2);
  const who = finding.querySelector('.react-who') as HTMLElement;
  expect(getComputedStyle(who).display).toBe('none');
  await userEvent.hover(mark.element());
  await expect.poll(() => getComputedStyle(who).display).not.toBe('none');
  expect(who.textContent).toBe('Made-up Reviewer');
  await userEvent.unhover(mark.element());

  await screen.rerender(
    <View state={apply(before, { type: 'reaction', chat: 'c', reaction: given('found', 'b', 5) })} />,
  );
  const both = page
    .elementLocator(finding)
    .getByRole('button', { name: 'Thumbs-up from Made-up Reviewer and Made-up Member' });
  await expect
    .poll(() => [...both.element().children].slice(0, 2).map((c) => c.textContent))
    .toEqual(['👍', '2']);
  (both.element() as HTMLElement).focus();
  const names = finding.querySelector('.react-who') as HTMLElement;
  await expect.poll(() => getComputedStyle(names).display).not.toBe('none');
  expect(names.textContent).toBe('Made-up ReviewerMade-up Member');
  // Names, never a tooltip.
  expect(document.querySelector('[role="tooltip"]')).toBeNull();
});

test('a turn’s steps show on its name line on hover or focus, and stay for a failed step or a working agent', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: {
      fine: turn('fine', 'a', 0),
      broke: turn('broke', 'b', 1),
      busy: turn('busy', 'c', 2, { status: 'running' }),
    },
    messages: [
      tool('t1', 'fine', 'Read'),
      said('m1', 'fine', 'Made-up fine words'),
      tool('t2', 'broke', 'Bash', { status: 'error' }),
      said('m2', 'broke', 'Made-up words after a failure'),
      tool('t3', 'busy', 'Read'),
      tool('t4', 'busy', 'Bash', { status: 'running' }),
    ],
  };
  const screen = await render(<View state={state} />);
  const steps = (agent: string) =>
    document.querySelector(`.said[data-agent="${agent}"] .steps-button`) as HTMLElement;
  await expect.element(screen.getByText('Made-up fine words')).toBeVisible();
  const shown = (agent: string) => getComputedStyle(steps(agent)).opacity === '1';
  expect(shown('a')).toBe(false);
  expect(steps('b').textContent).toContain('1 failed');
  expect(shown('b')).toBe(true);
  expect(shown('c')).toBe(true);

  await userEvent.hover(screen.getByText('Made-up fine words').element());
  await expect.poll(() => shown('a')).toBe(true);
  await userEvent.unhover(screen.getByText('Made-up fine words').element());
  await expect.poll(() => shown('a')).toBe(false);
  // By keyboard, as a person reaches it: focus set by script after the pointer was used is no keyboard focus.
  // Safari's Tab skips buttons unless the person turns that on; Option+Tab reaches them.
  const tab = server.browser === 'webkit' ? '{Alt>}{Tab}{/Alt}' : '{Tab}';
  for (let i = 0; i < 10 && document.activeElement !== steps('a'); i++) await userEvent.keyboard(tab);
  expect(document.activeElement).toBe(steps('a'));
  await expect.poll(() => shown('a')).toBe(true);
  // It opens the steps under the name line.
  await userEvent.click(steps('a'));
  await expect.element(screen.getByRole('button', { name: /^Read a file/ })).toBeVisible();
});

test('a turn that only ran steps shows them without hover; one with words still hides them', async () => {
  const state: ChatState = {
    ...empty,
    members,
    turns: { quiet: turn('quiet', 'a', 0), said: turn('said', 'b', 1) },
    messages: [
      tool('t1', 'quiet', 'Read'),
      tool('t2', 'quiet', 'Bash'),
      tool('t3', 'said', 'Read'),
      said('m1', 'said', 'Made-up words'),
    ],
  };
  const screen = await render(
    <>
      <p>Made-up place to point at</p>
      <View state={state} />
    </>,
  );
  // Nothing pointed at the messages: the pointer rests on the line above them.
  await userEvent.hover(screen.getByText('Made-up place to point at').element());
  const steps = (agent: string) =>
    document.querySelector(`.said[data-agent="${agent}"] .steps-button`) as HTMLElement;
  await expect.poll(() => !!steps('b')).toBe(true);
  // The steps are all the quiet turn has: they show with nothing pointed at.
  expect(getComputedStyle(steps('a')).opacity).toBe('1');
  expect(getComputedStyle(steps('b')).opacity).toBe('0');
});
