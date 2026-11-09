// The service is the edge here: fetch answers as it would, and live events arrive when the test says.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Run } from '../api';
import type { LiveEvent } from '../live';
import { ChatView } from './ChatView';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('../live', () => ({
  listen: (listener: (event: LiveEvent) => void) => {
    live.add(listener);
    return () => live.delete(listener);
  },
}));
afterEach(() => vi.unstubAllGlobals());

const json = (body: unknown) => new Response(JSON.stringify(body));
const members = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
  { agent: 'b', name: 'Made-up Member', role: 'member' },
];
// The lead's turn waits in the queue; the member has nothing.
const history = (run: Run) => ({
  chat: 'c',
  messages: [],
  turns: [{ id: 't', status: 'queued', why: null, ref: 'r', agent: 'a' }],
  approvals: [],
  members,
  run,
});

function Chat() {
  return (
    <ChatView
      chat={{ id: 'c', title: 'Made-up chat' }}
      agents={[]}
      onEditAgent={() => {}}
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
    />
  );
}

test('a paused chat holds its queued turn, says so, and Resume runs it again', async () => {
  const asked: string[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.startsWith('/api/chats')) asked.push(path);
    if (path.endsWith('/resume')) return json({ state: 'running' });
    return path.endsWith('/tasks') ? json({ tasks: [], waits: [] }) : json(history({ state: 'paused' }));
  });
  const screen = await render(<Chat />);
  await expect.element(screen.getByRole('status').filter({ hasText: 'You paused' })).toBeVisible();
  // The held turn isn't running, so the composer sends rather than stops.
  await expect.element(screen.getByRole('button', { name: 'Send' })).toBeInTheDocument();
  await screen.getByRole('button', { name: 'Summary' }).click();
  await expect.element(screen.getByText('Supervisor · Held')).toBeVisible();
  await expect.element(screen.getByText('Member · Paused')).toBeVisible();
  await screen.getByRole('button', { name: 'Summary' }).click();

  await screen.getByRole('button', { name: 'More' }).click();
  await screen.getByRole('menuitem', { name: 'Resume' }).click();
  // No live event comes: the answer alone shows the chat running.
  await expect.element(screen.getByText('You paused')).not.toBeInTheDocument();
  await expect.element(screen.getByRole('button', { name: 'Stop this chat now' })).toBeInTheDocument();
  expect(asked).toEqual(['/api/chats/c/resume']);
});

test('a Pause answer that arrives after a newer live change never undoes it', async () => {
  let release = (_: Response) => {};
  vi.stubGlobal('fetch', (path: string) => {
    if (path.endsWith('/pause')) return new Promise<Response>((done) => (release = done));
    return Promise.resolve(
      path.endsWith('/tasks') ? json({ tasks: [], waits: [] }) : json(history({ state: 'running' })),
    );
  });
  live.clear();
  const screen = await render(<Chat />);
  await screen.getByRole('button', { name: 'More' }).click();
  await screen.getByRole('menuitem', { name: 'Pause' }).click();
  for (const listener of live) listener({ type: 'run', chat: 'c', run: { state: 'paused' } });
  await expect.element(screen.getByText('You paused')).toBeVisible();
  release(json({ state: 'pausing' }));
  await new Promise((r) => setTimeout(r, 50));
  await expect.element(screen.getByText('You paused')).toBeVisible();
  await expect.element(screen.getByText('Pausing…')).not.toBeInTheDocument();
});
