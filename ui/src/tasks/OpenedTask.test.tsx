// An agent's task opened in the Tasks tab recovers from a refused load and keeps Back throughout (Vale, #121).
import { useState } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { PersonTask, Task } from '../api';
import type { LiveEvent } from '../live';
import type { Draft } from './model';
import { TasksTab } from './TasksTab';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('../live', () => ({
  listen: (on: (event: LiveEvent) => void) => {
    live.add(on);
    return () => live.delete(on);
  },
}));
afterEach(() => {
  vi.unstubAllGlobals();
  live.clear();
});

const agentTask = (number: number, what: string): Task => ({
  number,
  chat: 'c1',
  giver: 'mira',
  owner: 'sol',
  given_in: 't1',
  kind: 'work',
  what,
  state: 'working',
  reason: null,
  after: [],
  created_us: 0,
});
const question: PersonTask = {
  ...agentTask(3, 'What should the notice say?'),
  owner: null,
  kind: 'person',
  chat_title: 'Reconnect',
  for_agent: null,
  why: null,
  form: 'text',
  choices: [],
  files: [],
};

/** The tab as the app holds it: drafts and the opened task live outside it. */
function Tab() {
  const [opened, setOpened] = useState<number | undefined>(82);
  const [drafts, setDrafts] = useState<Record<number, Draft>>({ 3: { values: ['Kept draft'], notes: '' } });
  return (
    <>
      <button type="button" onClick={() => setOpened(83)}>
        Open 83
      </button>
      <TasksTab
        tasks={[question]}
        selected={3}
        onSelect={() => {}}
        drafts={drafts}
        onDraft={(n, d) => setDrafts((all) => ({ ...all, [n]: d }))}
        name={(a) => a ?? 'you'}
        onOpenFile={() => {}}
        onReference={() => {}}
        onSend={async () => []}
        onSettle={async () => {}}
        opened={
          opened === undefined ? undefined : { chat: 'c1', task: opened, onBack: () => setOpened(undefined) }
        }
      />
    </>
  );
}

test('a refused load says why with Back; a later update shows the task, another task opens, and Back keeps drafts', async () => {
  let refuse = true;
  vi.stubGlobal('fetch', async (path: string) => {
    if (path !== '/api/chats/c1/tasks') return new Response('{}');
    if (refuse) {
      const error = { error: 'Tasks failed', why: 'made-up refusal', ref: 'made-up-ref' };
      return new Response(JSON.stringify(error), { status: 500 });
    }
    return new Response(
      JSON.stringify({ tasks: [agentTask(82, 'Fix it'), agentTask(83, 'Check it')], waits: [] }),
    );
  });
  const screen = await render(<Tab />);
  await expect.element(screen.getByText('made-up refusal')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Back' })).toBeVisible();

  refuse = false;
  for (const on of live) on({ type: 'tasks', chat: 'c1', tasks: [agentTask(82, 'Fix it')], waits: [] });
  await expect.element(screen.getByRole('heading', { name: 'Fix it' })).toBeVisible();
  await expect.element(screen.getByText('made-up refusal')).not.toBeInTheDocument();

  await screen.getByRole('button', { name: 'Open 83' }).click();
  await expect.element(screen.getByRole('heading', { name: 'Check it' })).toBeVisible();

  await screen.getByRole('button', { name: 'Back' }).click();
  await expect.element(screen.getByRole('heading', { name: 'What should the notice say?' })).toBeVisible();
  await expect.element(screen.getByRole('textbox', { name: /^Your answer/ })).toHaveValue('Kept draft');
});
