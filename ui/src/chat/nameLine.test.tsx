// The name line above a message's bubble in the chat (the owner's call of 2026-11-18): the name, the time, and New on a
// reply said while the person was away. Nothing else: the steps stay in the agent's panel, and a turn with nothing to say
// or act on draws nothing, counted nowhere.
import { useRef } from 'react';
import { afterEach, expect, test } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { agents, custom, opened, round, task, tool } from './madeUp';
import { toThread } from './messages';
import { chat, noActions, View as Recorded } from './recorded';
import type { Approval, ChatState } from './state';
import { Thread } from './Thread';

afterEach(async () => {
  await page.viewport(1280, 900);
});

/** A round's chat drawn whole, every thread open, at `width` and `height`. */
function View({ state, width = 900, height = 900 }: { state: ChatState; width?: number; height?: number }) {
  const cache = useRef(new Map());
  return (
    <div style={{ height, width, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, [], cache.current, opened).items}
        running={false}
        actions={noActions}
        empty={false}
        questions={{
          tasks: state.tasks,
          drafts: {},
          onDraft: () => {},
          onSend: async () => null,
          name: (a) => a ?? '',
        }}
      />
    </div>
  );
}

/** What a name line holds after the name: "time" for the time, else each piece's words. */
const pieces = (line: Element) =>
  [...line.querySelectorAll('.name-line-meta > *, .name-line > :not(.name-line-name, .name-line-meta)')].map(
    (p) => (p.tagName === 'TIME' ? 'time' : p.textContent),
  );
const blockOf = (agent: string) => document.querySelector(`.thread-messages .said[data-agent="${agent}"]`);

/** The members' turns in the ask's thread, where a member's message named its task (0075). */
function inTheAsk(state: ChatState): ChatState {
  for (const id of ['rowan', 'kit']) state.turns[id] = { ...state.turns[id], topic: state.turns.ask.topic };
  return state;
}

/** Rowan's turn on task 1 with two steps before its words, one failed, cut when its task was cancelled. */
function cancelled(): ChatState {
  const state = inTheAsk(round());
  state.tasks = [task(1, 'rowan', 'ask', { state: 'cancelled' }), task(2, 'kit', 'ask')];
  state.turns.rowan = { ...state.turns.rowan, status: 'stopped', end: 'cancelled' };
  const at = state.messages.findIndex((m) => m.turn === 'rowan' && m.role === 'assistant');
  state.messages.splice(
    at,
    0,
    tool('rowan', 'rowan-read', 'Read'),
    tool('rowan', 'rowan-bash', 'Bash', { status: 'error' }),
  );
  return state;
}

test('a reply that worked on a task, ran steps with one failed and ended cancelled names only itself and the time', async () => {
  await render(<View state={cancelled()} />);
  await expect.element(page.getByText('Cursor fixed')).toBeVisible();
  const rowan = blockOf('rowan') as Element;
  expect(rowan.querySelector('.name-line-name')?.textContent).toBe('Rowan');
  expect(pieces(rowan.querySelector('.name-line') as Element)).toEqual(['time']);
  // Its steps are one line in its bubble, above its words (the owner's call of 2026-11-19), not on its name line.
  expect([...rowan.querySelectorAll('.bubble > .work button')].map((w) => w.textContent?.trim())).toEqual([
    'Read 1 file, ran 1 command',
  ]);
});

test('a main message says no state, mark or PR; the lead is not called Supervisor; a result does not say Result', async () => {
  for (const name of ['askDone', 'reviewAndFix'] as const) {
    await render(<Recorded key={name} state={chat(name)} />);
    await expect
      .poll(() => document.querySelectorAll('.thread-messages .name-line').length)
      .toBeGreaterThan(2);
    const lines = [...document.querySelectorAll('.thread-messages .name-line')];
    // The person's own ask keeps its size, which is theirs to change.
    const extra = lines
      .map((l) => [l.querySelector('.name-line-name')?.textContent, ...pieces(l).filter((p) => p !== 'time')])
      .filter((l) => l.length > 1 && !(l[0] === 'You' && l.length === 2 && /Fix|Plan/.test(l[1] ?? '')));
    expect(extra, name).toEqual([]);
    expect(
      document.querySelectorAll('.thread-messages .name-line :is(.main-state, .mark, svg):not(.ask-size *)'),
      name,
    ).toHaveLength(0);
  }
});

test('a working turn with no words is a reply in its thread from its first step; its words show as they come', async () => {
  const state = inTheAsk(round());
  state.turns.rowan = { ...state.turns.rowan, status: 'running' };
  state.messages = state.messages.filter((m) => !(m.turn === 'rowan' && m.role === 'assistant'));
  state.messages.push(tool('rowan', 'rowan-run', 'Bash', { status: 'running' }));
  // The thread of Mira's answer to the ask, a main message of its own (0078).
  const main = (s: ChatState) => {
    const t = toThread(s, agents, [], new Map(), opened);
    return { t, fold: custom(t.items.find((i) => i.id === 'a-ask')).thread };
  };
  const before = main(state);
  expect(before.t.items.map((i) => i.id)).toContain('a-rowan');
  // Kit's words and Rowan's at work.
  expect(before.fold).toMatchObject({ topic: 'b:ask', count: 2 });
  expect(before.t.find('rowan-run')?.id).toBe('a-rowan');

  const screen = await render(<View state={state} />);
  await expect.element(page.getByText('Checked')).toBeVisible();
  const working = blockOf('rowan')?.querySelector('.bubble > .work');
  expect(working?.querySelector('button')?.textContent).toMatch(/^Working/);
  // Its latest step, under the working line.
  expect(working?.querySelector('[data-slot="collapsible"] > p')?.textContent).toBe('Running a command');

  // Its first words: in its bubble, after its working line, on the same name line.
  const streaming = { ...state, partial: { rowan: 'Looking at the cursor' } };
  await screen.rerender(<View state={streaming} />);
  await expect.element(page.getByText('Looking at the cursor')).toBeVisible();
  expect(pieces(blockOf('rowan')?.querySelector('.name-line') as Element)).toEqual(['time']);
  expect(main(streaming).fold?.count).toBe(2);
});

test('an ended turn that only ran steps keeps its line as a reply; a turn dropped before it ran draws nothing', () => {
  const state = inTheAsk(round());
  state.messages = state.messages.filter((m) => !(m.turn === 'rowan' && m.role === 'assistant'));
  state.messages.push(tool('rowan', 'rowan-read', 'Read'), tool('rowan', 'rowan-edit', 'Edit'));
  const t = toThread(state, agents, [], new Map(), opened);
  const pieces = custom(t.items.find((i) => i.id === 'a-rowan')).pieces;
  expect(pieces?.map((p) => ('run' in p ? p.run.label : 'words' in p && p.words))).toEqual([
    'Read 1 file, edited 1 file',
  ]);
  // In the thread of Mira's answer (0078): Rowan's line and Kit's words.
  expect(custom(t.items.find((i) => i.id === 'a-ask')).thread?.count).toBe(2);

  // Stopped by Gathr for a reason of its own before it wrote or ran anything: nothing to show, nothing to act on.
  state.messages = state.messages.filter((m) => m.turn !== 'rowan' || m.role !== 'assistant');
  state.turns.rowan = { ...state.turns.rowan, status: 'stopped', end: 'cancelled' };
  const dropped = toThread(state, agents, [], new Map(), opened);
  expect(dropped.every.map((i) => i.id)).toContain('a-rowan');
  expect(dropped.all.map((i) => i.id)).not.toContain('a-rowan');
  expect(custom(dropped.items.find((i) => i.id === 'a-ask')).thread?.count).toBe(1);
});

test('a turn with no words still shows what the person can act on: Retry with why it ended, an approval, a question', async () => {
  const state = inTheAsk(round());
  state.messages = state.messages.filter(
    (m) => !(m.role === 'assistant' && ['rowan', 'kit'].includes(m.turn ?? '')),
  );
  state.messages.push(tool('rowan', 'rowan-read', 'Read'));
  state.turns.rowan = { ...state.turns.rowan, status: 'failed', why: 'the engine exited', ref: '5236079d' };
  state.turns.kit = { ...state.turns.kit, status: 'running' };
  const approval: Approval = { request: 'r1', turn: 'kit', tool: 'Bash', input: { command: 'npm test' } };
  state.approvals = [approval];
  // Lane's question to the person, asked in a turn that said nothing else.
  state.turns.q = { ...state.turns.direct, id: 'q', agent: 'lane', position: 9, topic: 't:5' };
  state.tasks = [
    ...state.tasks,
    task(5, 'lane', 'q', { kind: 'person', giver: 'lane', state: 'ready', what: 'Keep the old recipes?' }),
  ];
  state.messages.push(tool('q', 'q-ask', 'mcp__gathr__ask_person'));
  await render(<View state={state} />);
  await expect.element(page.getByRole('button', { name: 'Retry', exact: true })).toBeVisible();
  await expect.element(page.getByRole('button', { name: 'Reference 5236079d' })).toBeVisible();
  expect(pieces(blockOf('rowan')?.querySelector('.name-line') as Element)).toEqual(['time']);
  await expect.element(page.getByRole('button', { name: 'Allow', exact: true })).toBeVisible();
  expect(pieces(blockOf('kit')?.querySelector('.name-line') as Element)).toEqual(['time']);
  await expect.element(page.getByText('Keep the old recipes?').first()).toBeVisible();
});

test('the 360 column never scrolls sideways, whatever a reply’s turn did', async () => {
  await page.viewport(560, 900);
  const state = cancelled();
  state.tasks[0] = {
    ...state.tasks[0],
    what: 'Made-up task with a title long enough to fill the line by itself',
  };
  state.turns.rowan = { ...state.turns.rowan, end: 'restart' };
  for (let i = 0; i < 12; i++)
    state.messages.splice(7, 0, tool('rowan', `rowan-${i}`, 'Bash', { status: 'error' }));
  await render(<View state={state} width={360} />);
  await expect.element(page.getByText('Cursor fixed')).toBeVisible();
  const view = document.querySelector('.thread-viewport') as HTMLElement;
  expect(view.scrollWidth - view.clientWidth).toBe(0);
  for (const line of document.querySelectorAll('.thread-messages .name-line'))
    expect(line.scrollWidth - line.clientWidth, line.textContent ?? '').toBe(0);
});

/** The round with a turn of Kit's that nothing ties, at work and with no words yet: the chat's newest. */
function late(): ChatState {
  const state = round();
  state.turns.late = {
    ...state.turns.kit,
    id: 'late',
    ref: 'late',
    task: undefined,
    position: 9,
    topic: 'b:late',
  };
  state.turns.late.status = 'running';
  state.messages.push(tool('late', 'late-run', 'Bash', { status: 'running' }));
  return state;
}

test('a working turn’s first words show in place: the chat at its foot follows them, one scrolled up stays', async () => {
  const words = { ...late(), partial: { late: 'Made-up first words '.repeat(30) } };
  const viewport = () => document.querySelector('.thread-viewport') as HTMLElement;
  const fromFoot = () => viewport().scrollHeight - viewport().clientHeight - viewport().scrollTop;
  // At its foot.
  const screen = await render(<View state={late()} height={420} />);
  await expect.element(page.getByText('It does')).toBeVisible();
  await expect.poll(() => viewport().scrollHeight > viewport().clientHeight).toBe(true);
  viewport().scrollTop = viewport().scrollHeight;
  await expect.poll(fromFoot).toBeLessThan(2);
  await screen.rerender(<View state={words} height={420} />);
  await expect.element(page.getByText(/^Made-up first words/)).toBeInTheDocument();
  await expect.poll(fromFoot).toBeLessThan(2);

  // Scrolled up: what the person reads stays where it was.
  await screen.rerender(<View state={late()} height={420} />);
  await expect.poll(() => !!blockOf('kit')?.textContent?.includes('Made-up first words')).toBe(false);
  // As the person scrolls: the chat hears it, and stops following its foot.
  await userEvent.wheel(viewport(), { delta: { y: -100_000 } });
  const first = page.getByText('Fix the reconnect').element();
  await expect.poll(() => viewport().scrollTop).toBe(0);
  // The chat's own sign that it heard the scroll: its arrow to the foot is offered. Before it, WebKit could still
  // follow the foot (one run in three).
  await expect.poll(() => !!document.querySelector('.thread-bottom:not(:disabled)')).toBe(true);
  const top = first.getBoundingClientRect().top;
  await screen.rerender(<View state={words} height={420} />);
  await expect.element(page.getByText(/^Made-up first words/)).toBeInTheDocument();
  expect(viewport().scrollTop).toBe(0);
  expect(first.getBoundingClientRect().top - top).toBe(0);
});
