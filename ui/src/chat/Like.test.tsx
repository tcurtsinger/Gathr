// The person's like (0057) and the row on a bubble's corner that holds it: shown on hover or keyboard focus, the
// button and the count give and take it back, "You" first in the names, which messages offer it, no hover text, and a
// row that never covers a word of the bubble's first line.
import { useRef } from 'react';
import { beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Message, PlanSummary, Task, Turn } from '../api';
import { toThread } from './messages';
import { apply, type ChatState, empty } from './state';
import { type Actions, Thread } from './Thread';

const members = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' as const },
  { agent: 'b', name: 'Made-up Member', role: 'member' as const },
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
const said = (id: string, turnId: string | null, body: string, more: Partial<Message> = {}): Message => ({
  id,
  turn: turnId,
  time: `2026-11-08T10:${String(minute++ % 60).padStart(2, '0')}:00Z`,
  role: 'assistant',
  kind: 'text',
  body,
  ...more,
});
const LONG =
  'Made-up long words that run on past the column, so the bubble takes the whole width and its first line ' +
  'reaches the corner where the row sits, and then a few more made-up words to wrap again.';
const question = (number: number, turnId: string, more: Partial<Task>): Task =>
  ({
    number,
    chat: 'c',
    giver: 'a',
    owner: null,
    given_in: turnId,
    kind: 'person',
    what: `Made-up question ${number}?`,
    state: 'ready',
    reason: null,
    after: [],
    created_us: Date.parse('2026-11-08T11:00:00Z') * 1000,
    form: 'choice',
    choices: [{ label: 'Made-up pick' }, { label: 'Made-up other' }],
    ...more,
  }) as Task;

/** A chat with one of each kind of message the row's rule decides on. */
function room(): ChatState {
  const plan: PlanSummary = {
    id: 1,
    ask: 'p',
    title: 'Made-up plan',
    lead: 'a',
    state: 'waiting',
    versions: [
      {
        id: 1,
        n: 1,
        message: 'v1',
        parts: 2,
        check_line: null,
        sent: '2026-11-08T12:00:00Z',
        decided: null,
        decided_line: null,
      },
    ],
  };
  return {
    ...empty,
    members,
    turns: {
      you: turn('you', 'a', 0),
      long: turn('long', 'a', 1),
      short: turn('short', 'b', 2),
      asks: turn('asks', 'a', 3),
      open: turn('open', 'a', 4),
      plan: turn('plan', 'a', 5),
      busy: turn('busy', 'b', 6, { status: 'running' }),
    },
    messages: [
      said('p', 'you', 'Made-up words of the person', { role: 'user' }),
      said('m-long', 'long', LONG),
      said('m-short', 'short', 'Done.'),
      said('g', null, 'Made-up line of Gathr', { role: 'gathr' }),
      said('m-asks', 'asks', 'Made-up words with a pick'),
      said('m-open', 'open', 'Made-up words with no pick'),
      said('v1', 'plan', 'Made-up plan words'),
      said('t1', 'busy', 'Made-up talk to the lead', { to: 'a' }),
    ],
    tasks: [question(1, 'asks', {}), question(2, 'open', { form: 'text', choices: [] })],
    plans: [plan],
    partial: { busy: 'Made-up words still stream' },
  };
}

function View({ state, like, width = 1440 }: { state: ChatState; like?: Actions['like']; width?: number }) {
  const cache = useRef(new Map());
  const none = () => {};
  const actions: Actions = {
    openTask: none,
    openAgent: none,
    diagnostics: none,
    reference: none,
    signIn: none,
    retry: none,
    resend: none,
    carryOn: none,
    approve: none,
    like,
  };
  return (
    <div style={{ height: 1400, width, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, [], cache.current).items}
        running={false}
        actions={actions}
        empty={false}
      />
    </div>
  );
}

// The window's own size: the row's corner is measured as the person sees it.
beforeEach(async () => {
  await page.viewport(1440, 900);
});

/** The message whose words hold `words`, its row and its Like button, if it has them. */
function message(words: string) {
  const said = page.getByText(words, { exact: true }).element().closest('.said') as HTMLElement;
  const row = said.querySelector(':scope > .said-main > .bubble > .message-actions') as HTMLElement | null;
  const button = row?.querySelector('button') as HTMLButtonElement | undefined;
  return { said, row, button };
}

test('the row shows while its message is pointed at or holds focus, and not otherwise', async () => {
  const screen = await render(
    <>
      <p>Made-up place to point at</p>
      <View state={room()} />
    </>,
  );
  await expect.element(screen.getByText('Done.')).toBeVisible();
  await userEvent.hover(screen.getByText('Made-up place to point at').element());
  const { row, button } = message('Done.');
  const shown = () => getComputedStyle(row as HTMLElement).opacity;
  expect(shown()).toBe('0');
  await userEvent.hover(screen.getByText('Done.').element());
  await expect.poll(shown).toBe('1');
  // Another message pointed at shows its own row, not this one.
  await userEvent.hover(screen.getByText(LONG).element());
  await expect.poll(shown).toBe('0');
  await userEvent.hover(screen.getByText('Made-up place to point at').element());
  (button as HTMLButtonElement).focus();
  await expect.poll(shown).toBe('1');
  expect(button?.getAttribute('aria-label')).toBe('Like');
  (button as HTMLButtonElement).blur();
  await expect.poll(shown).toBe('0');
});

test('the button and the count give the like and take it back, and the names say "You" first', async () => {
  const like = vi.fn();
  const before: ChatState = {
    ...room(),
    reactions: [{ message: 'm-short', agent: 'a', time: '2026-11-08T10:30:00Z' }],
  };
  const screen = await render(<View state={before} like={like} />);
  await expect.element(screen.getByText('Done.')).toBeVisible();
  const button = message('Done.').button as HTMLButtonElement;
  expect(button.getAttribute('aria-pressed')).toBe('false');
  await userEvent.click(button);
  expect(like).toHaveBeenLastCalledWith('m-short', [], true);
  // The agent's thumbs-up: the person's press joins it.
  const count = () => message('Done.').said.querySelector('.react') as HTMLButtonElement;
  expect(count().getAttribute('aria-pressed')).toBe('false');
  await userEvent.click(count());
  expect(like).toHaveBeenLastCalledWith('m-short', [], true);

  const mine = { message: 'm-short', agent: 'person', time: '2026-11-08T10:31:00Z' };
  const given = apply(before, { type: 'reaction', chat: 'c', reaction: mine });
  await screen.rerender(<View state={given} like={like} />);
  await expect.poll(() => count().getAttribute('aria-label')).toBe('Thumbs-up from You and Made-up Lead');
  expect(count().getAttribute('aria-pressed')).toBe('true');
  expect(count().hasAttribute('data-mine')).toBe(true);
  // The person's own in the attention blue.
  const blue = document.createElement('span');
  blue.style.color = 'var(--attention)';
  document.body.append(blue);
  expect(getComputedStyle(count()).color).toBe(getComputedStyle(blue).color);
  blue.remove();
  expect([...count().children].slice(0, 2).map((c) => c.textContent)).toEqual(['👍', '2']);
  expect((count().querySelector('.react-who') as HTMLElement).textContent).toBe('YouMade-up Lead');
  expect((message('Done.').button as HTMLButtonElement).getAttribute('aria-pressed')).toBe('true');
  await userEvent.click(count());
  expect(like).toHaveBeenLastCalledWith('m-short', [], false);
  await userEvent.click(message('Done.').button as HTMLButtonElement);
  expect(like).toHaveBeenLastCalledWith('m-short', [], false);

  // Taken back, live: the count is the agent's alone again.
  const back = apply(given, { type: 'reaction', chat: 'c', reaction: mine, removed: true });
  await screen.rerender(<View state={back} like={like} />);
  await expect.poll(() => count().getAttribute('aria-label')).toBe('Thumbs-up from Made-up Lead');
  expect(count().hasAttribute('data-mine')).toBe(false);
});

test('an agent’s words, a pick, a waiting plan and talk offer the like; nothing else does', async () => {
  const like = vi.fn();
  const screen = await render(<View state={room()} like={like} />);
  await expect.element(screen.getByText('Done.')).toBeVisible();
  const offers = (words: string) => !!message(words).button;
  expect(offers(LONG)).toBe(true);
  expect(offers('Made-up talk to the lead')).toBe(true);
  // The person's own, Gathr's line, a question with no pick and words still streaming offer none.
  expect(offers('Made-up words of the person')).toBe(false);
  expect(document.querySelector('.said[data-gathr] .message-actions')).toBeNull();
  expect(offers('Made-up words with no pick')).toBe(false);
  expect(offers('Made-up words still stream')).toBe(false);
  // A pick: the like answers the question with it.
  await userEvent.click(message('Made-up words with a pick').button as HTMLButtonElement);
  expect(like).toHaveBeenLastCalledWith('m-asks', [1], true);
  // A version waiting for the person: the like approves it. Once decided, it offers none.
  const chip = document.querySelector('.plan-chip')?.closest('.said') as HTMLElement;
  const approve = chip.querySelector('.message-actions button') as HTMLButtonElement;
  await userEvent.click(approve);
  expect(like).toHaveBeenLastCalledWith('v1', [], true);
  const decided = room();
  decided.plans = [{ ...decided.plans[0], state: 'approved' }];
  decided.plans[0].versions = [{ ...decided.plans[0].versions[0], decided: 'approved' }];
  decided.tasks = decided.tasks.map((t) => ({ ...t, state: 'done' as const }));
  await screen.rerender(<View state={decided} like={like} />);
  await expect
    .poll(() => document.querySelector('.plan-chip')?.closest('.said')?.querySelector('.message-actions'))
    .toBeNull();
  // An answered question's words are an ordinary message: the like is offered there.
  expect(offers('Made-up words with no pick')).toBe(true);
  // No hover text anywhere.
  expect(document.querySelector('[title]')).toBeNull();
  expect(document.querySelector('[role="tooltip"]')).toBeNull();
});

for (const width of [1440, 360]) {
  test(`the row never covers a word of the bubble’s first line, at ${width} wide`, async () => {
    // The 360 column is the chat beside an open panel at 1000 wide.
    await page.viewport(width === 360 ? 1000 : 1440, 900);
    const screen = await render(<View state={room()} width={width} />);
    await expect.element(screen.getByText('Done.')).toBeVisible();
    for (const words of [LONG, 'Done.', 'Made-up talk to the lead']) {
      const { said, row } = message(words);
      await userEvent.hover(screen.getByText(words, { exact: true }).element());
      await expect.poll(() => getComputedStyle(row as HTMLElement).opacity).toBe('1');
      const bubble = (row as HTMLElement).parentElement as HTMLElement;
      const first =
        bubble.getBoundingClientRect().top + Number.parseFloat(getComputedStyle(bubble).paddingTop);
      const below = (row as HTMLElement).getBoundingClientRect().bottom;
      expect(below, `${words}: the row reaches ${below - first}px into the first line`).toBeLessThanOrEqual(
        first,
      );
      // On its own bubble's corner, never past the column.
      const column = said.getBoundingClientRect();
      expect((row as HTMLElement).getBoundingClientRect().right).toBeLessThanOrEqual(column.right + 6);
    }
  });
}
