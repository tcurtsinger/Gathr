// The service is the edge here: fetch answers as it would, and live events arrive when the test says.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Pauses } from '../api';
import type { LiveEvent } from '../live';
import { ChatView } from './ChatView';
import { usePauses } from './pauses';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('../live', () => ({
  listen: (listener: (event: LiveEvent) => void) => {
    live.add(listener);
    return () => live.delete(listener);
  },
}));
afterEach(() => {
  vi.unstubAllGlobals();
  live.clear();
});

const json = (body: unknown) => new Response(JSON.stringify(body));
const running: Pauses = { pauses: [], usage: [] };
const paused = (scope: 'app' | 'chat' | 'agent', target = ''): Pauses => ({
  pauses: [{ scope, target, state: 'paused', by: null }],
  usage: [],
});
// The lead's turn is held by the pause; the member has nothing.
const history = {
  chat: 'c',
  messages: [],
  turns: [{ id: 't', status: 'queued', held: 'pause', why: 'paused', ref: 'r', agent: 'a' }],
  approvals: [],
  members: [
    { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
    { agent: 'b', name: 'Made-up Member', role: 'member' },
  ],
};

// What the service's list reads now; a test changes it as the service would.
let listed: Pauses = running;

/** The service as these tests need it; each POST is kept with its body. */
function service(pauses: Pauses, answer: (path: string) => Promise<Response>) {
  listed = pauses;
  const posted: { path: string; body: unknown }[] = [];
  vi.stubGlobal('fetch', (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.startsWith('/api/pauses')) {
      posted.push({ path, body: JSON.parse(String(init.body)) });
      return answer(path);
    }
    if (path === '/api/pauses') return Promise.resolve(json(listed));
    return Promise.resolve(path.endsWith('/tasks') ? json({ tasks: [], waits: [] }) : json(history));
  });
  return posted;
}

const noFailure = () => {};

/**
 * The open Summary, to read its agents' states from. In WebKit, vitest's toBeVisible counts anything inside
 * a closed <details> as hidden unless it is the <summary> itself, so each state is read as the dialog's text.
 */
const summary = page.getByRole('dialog', { name: 'Summary' });

/** The chat as the app shows it, with the app's one list of pauses. */
function Chat() {
  const { pauses, act } = usePauses(noFailure);
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
      pauses={pauses}
      onPause={act}
      split
      onSplit={() => {}}
      plan={false}
      onPlan={() => {}}
    />
  );
}

test('a chat the whole app paused holds its turn, says so, and Resume all resumes the app', async () => {
  const posted = service(paused('app'), async () => json(running));
  const screen = await render(<Chat />);
  await expect.element(screen.getByRole('status').filter({ hasText: 'You paused' })).toBeVisible();
  // The held turn isn't under way, so the composer sends rather than stops.
  await expect.element(screen.getByRole('button', { name: 'Send' })).toBeInTheDocument();
  await screen.getByRole('button', { name: 'Summary' }).click();
  await expect.element(summary).toMatchTextContent('Supervisor · Held');
  await expect.element(summary).toMatchTextContent('Member · Paused');
  await screen.getByRole('button', { name: 'Summary' }).click();

  await screen.getByRole('button', { name: 'More', exact: true }).click();
  await screen.getByRole('menuitem', { name: 'Resume all' }).click();
  // No live event comes: the answer alone shows the chat running.
  await expect.element(screen.getByText('You paused')).not.toBeInTheDocument();
  expect(posted).toEqual([{ path: '/api/pauses/resume', body: { scope: 'app' } }]);
});

test('a Pause answer that arrives after a newer live change never undoes it', async () => {
  let release = (_: Response) => {};
  service(running, () => new Promise<Response>((done) => (release = done)));
  const screen = await render(<Chat />);
  await screen.getByRole('button', { name: 'More', exact: true }).click();
  await screen.getByRole('menuitem', { name: 'Pause' }).click();
  listed = paused('chat', 'c');
  for (const listener of live) listener({ type: 'pauses', ...listed });
  await expect.element(screen.getByText('You paused')).toBeVisible();
  release(json({ pauses: [{ scope: 'chat', target: 'c', state: 'pausing', by: null }], usage: [] }));
  await new Promise((r) => setTimeout(r, 50));
  await expect.element(screen.getByText('You paused')).toBeVisible();
  await expect.element(screen.getByText('Pausing…')).not.toBeInTheDocument();
});

test('an agent paused from its row in Summary pauses that agent alone', async () => {
  const posted = service(running, async () => json(paused('agent', 'b')));
  const screen = await render(<Chat />);
  await screen.getByRole('button', { name: 'Summary' }).click();
  await expect.element(summary).toMatchTextContent('Member · Ready');
  await screen.getByRole('button', { name: 'More for Made-up Member' }).click();
  await screen.getByRole('menuitem', { name: 'Pause' }).click();
  await expect.poll(() => posted.length).toBe(1);
  await expect.element(summary).toMatchTextContent('Member · Paused');
  expect(posted).toEqual([{ path: '/api/pauses', body: { scope: 'agent', target: 'b' } }]);
  // The chat itself still runs.
  await expect.element(screen.getByText('You paused')).not.toBeInTheDocument();
});

test('two agents paused at once, with no live updates, both read paused once both answers are in', async () => {
  const answers: ((r: Response) => void)[] = [];
  service(running, () => new Promise<Response>((done) => answers.push(done)));
  const screen = await render(<Chat />);
  await screen.getByRole('button', { name: 'Summary' }).click();
  await screen.getByRole('button', { name: 'More for Made-up Lead' }).click();
  await screen.getByRole('menuitem', { name: 'Pause' }).click();
  await screen.getByRole('button', { name: 'More for Made-up Member' }).click();
  await screen.getByRole('menuitem', { name: 'Pause' }).click();
  await expect.poll(() => answers.length).toBe(2);
  const both: Pauses = {
    pauses: [...paused('agent', 'a').pauses, ...paused('agent', 'b').pauses],
    usage: [],
  };
  listed = both;
  answers[0](json(paused('agent', 'a')));
  answers[1](json(both));
  await expect.element(summary).toMatchTextContent('Supervisor · Held');
  await expect.element(summary).toMatchTextContent('Member · Paused');
});

test('two agents paused at once, reaching the service in reverse order with no live updates, both read paused', async () => {
  const answers: ((r: Response) => void)[] = [];
  service(running, () => new Promise<Response>((done) => answers.push(done)));
  const screen = await render(<Chat />);
  await screen.getByRole('button', { name: 'Summary' }).click();
  await screen.getByRole('button', { name: 'More for Made-up Lead' }).click();
  await screen.getByRole('menuitem', { name: 'Pause' }).click();
  await screen.getByRole('button', { name: 'More for Made-up Member' }).click();
  await screen.getByRole('menuitem', { name: 'Pause' }).click();
  await expect.poll(() => answers.length).toBe(2);
  // The member's pause reached the service first, so its answer lists the member alone.
  answers[1](json(paused('agent', 'b')));
  await expect.element(summary).toMatchTextContent('Member · Paused');
  const both: Pauses = {
    pauses: [...paused('agent', 'a').pauses, ...paused('agent', 'b').pauses],
    usage: [],
  };
  listed = both;
  answers[0](json(both));
  await expect.element(summary).toMatchTextContent('Supervisor · Held');
  await expect.element(summary).toMatchTextContent('Member · Paused');
});

/** The agents the app's list holds paused, and a button that pauses three at once. */
function PausedAgents() {
  const { pauses, act } = usePauses(noFailure);
  const pauseThree = () => {
    for (const target of ['a', 'b', 'c']) void act('pause', { scope: 'agent', target });
  };
  return (
    <>
      <button type="button" onClick={pauseThree}>
        Pause three
      </button>
      <output>{pauses.pauses.map((p) => p.target).join(' ') || 'none'}</output>
    </>
  );
}

test('with no live updates, an answer to an older action never replaces the newest one', async () => {
  const agents = (...targets: string[]): Pauses => ({
    pauses: targets.flatMap((t) => paused('agent', t).pauses),
    usage: [],
  });
  const answers: ((r: Response) => void)[] = [];
  const reads: ((r: Response) => void)[] = [];
  vi.stubGlobal('fetch', (_: string, init?: RequestInit) =>
    init?.method === 'POST'
      ? new Promise<Response>((done) => answers.push(done))
      : new Promise<Response>((done) => reads.push(done)),
  );
  const screen = await render(<PausedAgents />);
  reads[0](json(running));
  await expect.element(screen.getByText('none')).toBeVisible();
  await screen.getByRole('button', { name: 'Pause three' }).click();
  await expect.poll(() => answers.length).toBe(3);
  answers[2](json(agents('a', 'b', 'c')));
  await expect.element(screen.getByText('a b c')).toBeVisible();
  answers[0](json(agents('a')));
  answers[1](json(agents('a', 'b')));
  await new Promise((r) => setTimeout(r, 50));
  await expect.element(screen.getByText('a b c')).toBeVisible();
});

test('a turn waiting for Gathrs own engine says what is downloading', async () => {
  const waiting = {
    ...history,
    turns: [{ ...history.turns[0], held: 'codex', why: 'Codex is downloading' }],
  };
  vi.stubGlobal('fetch', (path: string) =>
    Promise.resolve(
      json(path === '/api/pauses' ? running : path.endsWith('/tasks') ? { tasks: [], waits: [] } : waiting),
    ),
  );
  const screen = await render(<Chat />);
  await expect.element(screen.getByText('Codex is downloading', { exact: true })).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Retry', exact: true })).not.toBeInTheDocument();
  // Completion clears the notice in the same reply slot.
  for (const listener of live)
    listener({ type: 'turn', chat: 'c', turn: 't', status: 'done', ref: 'r', agent: 'a' });
  await expect.element(screen.getByText('Codex is downloading', { exact: true })).not.toBeInTheDocument();
});
