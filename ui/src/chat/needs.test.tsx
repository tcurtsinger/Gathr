// What waits on the person is said once, on the status line (the owner: "Keep the line"): there is no panel above the
// message box. A press on "<agent> needs you" goes to what is needed: the oldest approval where it was asked, even in
// a thread the person folded (0075), else the plan waiting, else the question.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { History, Standing } from '../api';
import { ChatView } from './ChatView';
import askAtWork from './recorded/ask-at-work.json';
import questionDiscussed from './recorded/question-discussed.json';

vi.mock('../live', () => ({ listen: () => () => {} }));
afterEach(() => vi.unstubAllGlobals());

type Recorded = { history: History; work: { plans: { state: string }[] } };

/** The service saying an agent needs the person, as its `waiting` answer does. */
const needs = (agent: string): Standing => ({
  agent,
  name: 'Made-up',
  state: 'needs_you',
  words: 'Needs you',
  what: null,
  task: null,
  since_us: null,
  on: [],
  until_us: null,
  failing: false,
});

function serve(history: unknown, work: unknown, agents: Standing[] = []) {
  vi.stubGlobal('fetch', async (path: string) => {
    if (path.endsWith('/waiting')) return new Response(JSON.stringify({ lines: [], agents, gathr: [] }));
    const body = path.endsWith('/tasks') ? work : path.startsWith('/api/chats/') ? history : {};
    return new Response(JSON.stringify(body));
  });
}

const chat = (onOpenPlan = () => {}) => (
  <div style={{ height: 800, display: 'flex', flexDirection: 'column' }}>
    <ChatView
      chat={{ id: 'c', title: 'Made-up chat' }}
      agents={[]}
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
      onOpenPlan={onOpenPlan}
    />
  </div>
);

/** The line's own "needs you": the top line's Summary says it too, with other words. */
const needsYou = () =>
  page
    .elementLocator(document.querySelector('.status-slot') as Element)
    .getByRole('button', { name: /needs? you$/ });

test('an approval waiting in a folded thread: "needs you" on the line opens the thread at its card', async () => {
  const { history, work } = askAtWork as unknown as Recorded;
  const ann = history.turns.find((t) => t.topic === 't:2')?.id as string;
  const waiting = { turn: ann, request_id: 'r1', tool: 'Bash', input: { command: 'cargo test' } };
  serve({ ...history, approvals: [waiting] }, work);
  await render(chat());
  const card = () => document.querySelector('[data-approval="r1"]');
  await expect.poll(card).not.toBeNull();
  // Said once: the line names Ann, and no panel repeats it above the message box.
  await expect.element(needsYou()).toHaveTextContent('Ann needs you');
  expect(document.querySelector('[aria-label="Needs you"]')).toBeNull();
  // The person folds Ann's task's thread: the card goes with it, the line stays.
  const task = page.getByText('Add the tags table').element().closest('.said') as HTMLElement;
  const fold = task.querySelector('.said-foot .fold-line') as HTMLButtonElement;
  fold.click();
  await expect.poll(card).toBeNull();
  await needsYou().click();
  await expect.poll(card).not.toBeNull();
  expect(fold.getAttribute('aria-expanded')).toBe('true');
});

test('a plan waiting on the person: "needs you" on the line opens the plan at its newest version', async () => {
  const { history, work } = askAtWork as unknown as Recorded;
  const lead = history.members?.[0].agent as string;
  const waiting = { ...work, plans: work.plans.map((p) => ({ ...p, state: 'waiting' })) };
  serve(history, waiting, [needs(lead)]);
  const opened = vi.fn();
  await render(chat(opened));
  await expect.element(needsYou()).toHaveTextContent('Lead needs you');
  await needsYou().click();
  expect(opened).toHaveBeenCalledWith(1, 1);
});

test('a question waiting on the person: "needs you" on the line goes to the question', async () => {
  const { history, work } = questionDiscussed as unknown as Recorded;
  serve(history, work);
  await render(chat());
  await expect.element(needsYou()).toBeVisible();
  await needsYou().click();
  // Held as a search hit is: the message that asks, the one that says Needs you.
  await expect
    .poll(() => document.querySelector('[data-search-hit]')?.textContent ?? '')
    .toContain('Needs you');
});

test('with nothing to go to, the line is one button that opens the list', async () => {
  const { history, work } = askAtWork as unknown as Recorded;
  serve(history, work, [needs(history.members?.[0].agent as string)]);
  await render(chat());
  await expect.poll(() => document.querySelector('.status-line')?.textContent).toContain('Lead needs you');
  await (document.querySelector('.status-line') as HTMLButtonElement).click();
  await expect.element(page.getByRole('dialog', { name: 'Team' })).toBeVisible();
});
