// What waits on the person is said by the marks on the agents' faces and in Summary, the status panel (the owner,
// 2026-11-18: "Why do we need statuses in multiple places"): nothing above the message box says it. A press on a
// needs-you agent's row in Summary goes to what is needed: the oldest approval where it was asked, even in a thread the
// person folded (0075), else the plan waiting, else the question; with none of them in the chat it opens the agent.
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

const chat = (onOpenPlan = () => {}, onOpenAgent = (_: string) => {}) => (
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
      onOpenAgent={onOpenAgent}
    />
  </div>
);

const summary = () => page.getByRole('dialog', { name: 'Summary' });

/** Opens Summary and presses the row of the agent named, once it says the agent needs the person. */
async function pressRow(name: string) {
  await page.getByRole('button', { name: new RegExp(`^Summary: ${name} needs you`) }).click();
  const row = summary().getByRole('button', { name: new RegExp(`^${name} `) });
  await expect.element(row).toBeVisible();
  await row.click();
  await expect.element(summary()).not.toBeInTheDocument();
}

test('nothing above the message box says who needs the person', async () => {
  const { history, work } = askAtWork as unknown as Recorded;
  const ann = history.turns.find((t) => t.topic === 't:2')?.id as string;
  const waiting = { turn: ann, request_id: 'r1', tool: 'Bash', input: { command: 'cargo test' } };
  serve({ ...history, approvals: [waiting] }, work);
  await render(chat());
  await expect.poll(() => document.querySelector('[data-approval="r1"]')).not.toBeNull();
  await expect.element(page.getByRole('button', { name: /^Summary: Ann needs you/ })).toBeVisible();
  const footer = document.querySelector('.thread-footer') as HTMLElement;
  expect(footer.textContent).not.toContain('needs you');
  expect(document.querySelector('.status-slot, .status-line, [aria-label="Needs you"]')).toBeNull();
});

test('an approval waiting in a folded thread: the agent’s row in Summary opens the thread at its card', async () => {
  const { history, work } = askAtWork as unknown as Recorded;
  const ann = history.turns.find((t) => t.topic === 't:2')?.id as string;
  const waiting = { turn: ann, request_id: 'r1', tool: 'Bash', input: { command: 'cargo test' } };
  serve({ ...history, approvals: [waiting] }, work);
  const opened = vi.fn();
  await render(chat(undefined, opened));
  const card = () => document.querySelector('[data-approval="r1"]');
  await expect.poll(card).not.toBeNull();
  // The person folds Ann's task's thread: the card goes with it.
  const task = page.getByText('Add the tags table').element().closest('.said') as HTMLElement;
  const fold = task.querySelector('.said-foot .fold-line') as HTMLButtonElement;
  fold.click();
  await expect.poll(card).toBeNull();
  await pressRow('Ann');
  await expect.poll(card).not.toBeNull();
  expect(fold.getAttribute('aria-expanded')).toBe('true');
  await expect.poll(() => card()?.closest('[data-search-hit]')).not.toBeNull();
  expect(opened).not.toHaveBeenCalled();
});

test('a plan waiting on the person: the lead’s row opens the plan at its newest version', async () => {
  const { history, work } = askAtWork as unknown as Recorded;
  const lead = history.members?.[0].agent as string;
  const waiting = { ...work, plans: work.plans.map((p) => ({ ...p, state: 'waiting' })) };
  serve(history, waiting, [needs(lead)]);
  const plan = vi.fn();
  await render(chat(plan));
  await pressRow('Lead');
  expect(plan).toHaveBeenCalledWith(1, 1);
});

test('a question waiting on the person: the asker’s row goes to the question', async () => {
  const { history, work } = questionDiscussed as unknown as Recorded;
  serve(history, work);
  await render(chat());
  await pressRow('Lead');
  // Held as a search hit is: the message that asks, the one that says Needs you.
  await expect
    .poll(() => document.querySelector('[data-search-hit]')?.textContent ?? '')
    .toContain('Needs you');
});

test('with nothing in the chat to go to, a needs-you row opens the agent as every row does', async () => {
  const { history, work } = askAtWork as unknown as Recorded;
  const lead = history.members?.[0].agent as string;
  serve(history, work, [needs(lead)]);
  const opened = vi.fn();
  await render(chat(undefined, opened));
  await pressRow('Lead');
  expect(opened).toHaveBeenCalledWith(lead);
});
