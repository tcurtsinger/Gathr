// An approval waiting in a thread the person folded (0075): the dock above the message box still shows it, and its
// Show opens the thread at the card.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { History } from '../api';
import { ChatView } from './ChatView';
import askAtWork from './recorded/ask-at-work.json';

vi.mock('../live', () => ({ listen: () => () => {} }));
afterEach(() => vi.unstubAllGlobals());

test("the dock's Show opens a folded thread at the approval waiting in it", async () => {
  const { history, work } = askAtWork as unknown as { history: History; work: unknown };
  const ann = history.turns.find((t) => t.topic === 't:2')?.id as string;
  const waiting = { turn: ann, request_id: 'r1', tool: 'Bash', input: { command: 'cargo test' } };
  vi.stubGlobal('fetch', async (path: string) => {
    const body = path.endsWith('/tasks')
      ? work
      : path.startsWith('/api/chats/')
        ? { ...history, approvals: [waiting] }
        : {};
    return new Response(JSON.stringify(body));
  });
  await render(
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
      />
    </div>,
  );
  const card = () => document.querySelector('[data-approval="r1"]');
  await expect.poll(card).not.toBeNull();
  // The person folds Ann's task's thread: the card goes with it, the dock stays.
  const task = page.getByText('Add the tags table').element().closest('.said') as HTMLElement;
  const fold = task.nextElementSibling?.querySelector('.fold-line') as HTMLButtonElement;
  fold.click();
  await expect.poll(card).toBeNull();
  await page.getByRole('button', { name: 'Show' }).click();
  await expect.poll(card).not.toBeNull();
  expect(fold.getAttribute('aria-expanded')).toBe('true');
});
