// An ask's size sits at the end of the person's own message's name line (0061): a word that opens to the ask's two
// sizes in place, moved between with the arrow keys; choosing the other one asks the service to change it.
import { useRef } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Ask } from '../api';
import { type ChatMarks, MarksContext } from './faceMarks';
import { agents, round, task, turn } from './madeUp';
import { toThread } from './messages';
import type { ChatState } from './state';
import { type Actions, Thread } from './Thread';

afterEach(() => vi.unstubAllGlobals());

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
};

function View({ state, archived = false }: { state: ChatState; archived?: boolean }) {
  const cache = useRef(new Map());
  const marks: ChatMarks = { still: new Map(), turning: new Map(), latest: new Map(), archived };
  return (
    <MarksContext value={marks}>
      <div style={{ height: 900, display: 'flex' }}>
        <Thread
          messages={toThread(state, agents, [], cache.current).items}
          running={false}
          actions={actions}
          empty={false}
        />
      </div>
    </MarksContext>
  );
}

const ask = (id: string, more: Partial<Ask> = {}): Ask => ({
  id,
  title: 'Made-up ask',
  size: null,
  by: null,
  sized_in: null,
  held: false,
  ...more,
});

/** The made-up round, its first message (`ask`'s) an ask Mira sized a repair. */
function sized(more: Partial<Ask> = { size: 'repair', by: 'mira', sized_in: 'ask' }): ChatState {
  const state = round();
  const first = state.messages[0].id;
  state.tasks = state.tasks.map((t) => ({ ...t, ask: first }));
  state.asks = [ask(first, more)];
  return state;
}

/** The person's message's name line, by its words. */
function nameLineOf(words: string) {
  const said = page.getByText(words, { exact: true }).element().closest('.said');
  return said?.querySelector('.name-line') as HTMLElement;
}

/** The service, stubbed: each POST it gets, and what it answers. */
function service(answer: () => Response) {
  const sent: { path: string; body: unknown }[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    sent.push({ path, body: init?.body ? JSON.parse(String(init.body)) : undefined });
    return answer();
  });
  return sent;
}

test('the size is a word at the end of the ask’s own name line, with who sized it', async () => {
  const screen = await render(<View state={sized()} />);
  const word = screen.getByRole('button', { name: 'Size: Fix, by Mira' });
  await expect.element(word).toBeVisible();
  const line = nameLineOf('Fix the reconnect');
  expect(line.contains(word.element())).toBe(true);
  // Last on the line, after the time.
  expect(line.querySelector('.name-line-meta')?.lastElementChild?.contains(word.element())).toBe(true);
  expect(word.element().textContent).toBe('Fix');
  // No line of its own anywhere else.
  expect(document.querySelectorAll('.events')).toHaveLength(0);

  // The person's own size has no face; an ask nobody sized reads Plan.
  await screen.rerender(<View state={sized({ size: 'new', by: 'person' })} />);
  await expect.element(screen.getByRole('button', { name: 'Size: Plan, by you' })).toBeVisible();
  expect(nameLineOf('Fix the reconnect').querySelector('.face')).toBeNull();
  await screen.rerender(<View state={sized({})} />);
  await expect.element(screen.getByRole('button', { name: 'Size: Plan' })).toBeVisible();
});

test('the arrow keys move between the two sizes, and the pressed one is marked by more than colour', async () => {
  const screen = await render(<View state={sized()} />);
  const word = screen.getByRole('button', { name: 'Size: Fix, by Mira' });
  (word.element() as HTMLElement).focus();
  await userEvent.keyboard('{Enter}');
  const group = screen.getByRole('group', { name: 'Size' });
  await expect.element(group).toBeVisible();
  const fix = group.getByRole('button', { name: 'Just fix it.' });
  const plan = group.getByRole('button', { name: 'Plan it with me first.' });
  // Opened in place: focus on the pressed one.
  await expect.element(fix).toHaveFocus();
  await expect.element(fix).toHaveAttribute('aria-pressed', 'true');
  await expect.element(plan).toHaveAttribute('aria-pressed', 'false');
  // A tick shows on the pressed one alone.
  const tick = (el: Element) => getComputedStyle(el, '::before');
  expect(tick(fix.element()).visibility).toBe('visible');
  expect(tick(plan.element()).visibility).toBe('hidden');

  await userEvent.keyboard('{ArrowRight}');
  await expect.element(plan).toHaveFocus();
  await userEvent.keyboard('{ArrowRight}');
  await expect.element(fix).toHaveFocus();
  await userEvent.keyboard('{ArrowLeft}');
  await expect.element(plan).toHaveFocus();
  await userEvent.keyboard('{ArrowUp}');
  await expect.element(fix).toHaveFocus();

  // Escape closes it, and focus goes back to the word.
  await userEvent.keyboard('{Escape}');
  await expect.element(group).not.toBeInTheDocument();
  await expect.element(word).toHaveFocus();
});

test('choosing the other size asks the service and closes; the pressed one asks nothing', async () => {
  const sent = service(() => new Response(JSON.stringify(ask('x'))));
  const state = sized();
  const screen = await render(<View state={state} />);
  await screen.getByRole('button', { name: 'Size: Fix, by Mira' }).click();
  await screen.getByRole('button', { name: 'Just fix it.' }).click();
  await expect.element(screen.getByRole('group', { name: 'Size' })).not.toBeInTheDocument();
  await screen.getByRole('button', { name: 'Size: Fix, by Mira' }).click();
  await userEvent.keyboard('{ArrowDown}');
  await userEvent.keyboard('{Enter}');
  await expect
    .poll(() => sent)
    .toEqual([{ path: `/api/asks/${state.asks[0].id}/size`, body: { size: 'new' } }]);
  await expect.element(screen.getByRole('group', { name: 'Size' })).not.toBeInTheDocument();
});

test('a refused size shows why under the message, with its reference', async () => {
  const refusal = { error: 'Size failed', why: 'chat c is archived', ref: '01REFSIZE' };
  service(() => new Response(JSON.stringify(refusal), { status: 409 }));
  const screen = await render(<View state={sized()} />);
  await screen.getByRole('button', { name: 'Size: Fix, by Mira' }).click();
  await screen.getByRole('button', { name: 'Plan it with me first.' }).click();
  const alert = screen.getByRole('alert');
  await expect.element(alert).toHaveTextContent('Size failed01REFSIZEchat c is archived');
  await expect.element(alert.getByRole('button', { name: 'Reference 01REFSIZE' })).toBeVisible();
  // Under the person's own bubble.
  const said = page.getByText('Fix the reconnect', { exact: true }).element().closest('.said');
  expect(said?.contains(alert.element())).toBe(true);
});

test('an archived chat shows the size with no choices', async () => {
  const screen = await render(<View state={sized()} archived />);
  const line = nameLineOf('Fix the reconnect');
  await expect.element(page.getByText('Fix', { exact: true })).toBeVisible();
  expect(line.textContent).toContain('Fix');
  expect(line.querySelector('button')).toBeNull();
  expect(screen.getByRole('button', { name: /^Size/ }).elements()).toHaveLength(0);
});

test('the size shows before the lead has said anything', async () => {
  // Mira's first turn is still running and has written nothing; she has already given a task of the ask.
  const state = sized({});
  state.turns.ask = { ...turn('ask', 'mira'), status: 'running', position: 0 };
  state.messages = state.messages.filter((m) => !(m.turn === 'ask' && m.role === 'assistant'));
  state.tasks = [task(1, 'rowan', 'ask', { state: 'ready', ask: state.asks[0].id })];
  const screen = await render(<View state={state} />);
  await expect.element(screen.getByRole('button', { name: 'Size: Plan' })).toBeVisible();
});

test('two asks answered in one turn each carry their own size', async () => {
  const state = sized();
  const second = state.messages.find((m) => m.body === 'Keep the partial answer');
  state.asks.push(ask(second?.id ?? '', { size: 'new', by: 'person' }));
  const screen = await render(<View state={state} />);
  expect(
    nameLineOf('Fix the reconnect').contains(
      screen.getByRole('button', { name: 'Size: Fix, by Mira' }).element(),
    ),
  ).toBe(true);
  expect(
    nameLineOf('Keep the partial answer').contains(
      screen.getByRole('button', { name: 'Size: Plan, by you' }).element(),
    ),
  ).toBe(true);
});
