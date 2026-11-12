// The service is the edge here: fetch answers as it would, from made-up state, and live events are told.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { App } from './App';
import type { AppState } from './api';
import type { LiveEvent } from './live';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('./live', () => ({
  listen: (on: (event: LiveEvent) => void) => {
    live.add(on);
    return () => live.delete(on);
  },
}));
const tell = (event: LiveEvent) => {
  for (const on of live) on(event);
};
afterEach(() => vi.unstubAllGlobals());

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });
const standing = (state: AppState['state'], update: string | null = null): AppState => ({
  version: '9.9.8',
  state,
  update: update ? { version: update } : null,
  failed: null,
});

/** What the service says of its update now, what Update answers, and each Update sent. */
const made: { app: AppState; refuse: boolean; sent: unknown[] } = {
  app: standing('idle'),
  refuse: false,
  sent: [],
};
function service(app: AppState) {
  Object.assign(made, { app, refuse: false, sent: [] });
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/app') return json(made.app);
    if (path === '/api/app/update') {
      made.sent.push(JSON.parse(String(init?.body)));
      if (made.refuse)
        return json({ error: 'Update failed', why: 'made-up refusal', ref: 'made-up-ref' }, 409);
      made.app = standing('updating', '9.9.9');
      return json({ installing: '9.9.9' });
    }
    if (path === '/api/agents') return json({ agents: [] });
    if (path === '/api/projects') return json({ projects: [] });
    if (path === '/api/person-tasks') return json({ tasks: [], stale: [] });
    if (path === '/api/pauses') return json({ pauses: [], usage: [] });
    return json(path.startsWith('/api/chats') ? [] : {});
  });
}
const update = () => page.getByRole('button', { name: 'Update to 9.9.9' });

test('the Update icon shows only while a release is ready, and one click sends Update once', async () => {
  service(standing('idle'));
  await render(<App />);
  await expect.element(page.getByRole('button', { name: 'Settings' })).toBeVisible();
  await expect.element(update()).not.toBeInTheDocument();

  tell({ type: 'app', ...standing('ready', '9.9.9') });
  await update().click();
  await expect.element(page.getByText('Updating Gathr')).toBeVisible();
  await expect.element(update()).not.toBeInTheDocument();
  expect(made.sent).toEqual([{ install: true }]);
});

test('the first state is read at load, and again once the live socket missed events', async () => {
  service(standing('ready', '9.9.9'));
  await render(<App />);
  await expect.element(update()).toBeVisible();

  // A hand-over drops the socket: what the new service says replaces what was shown.
  made.app = { ...standing('idle'), version: '9.9.9' };
  tell({ type: 'lagged' });
  await expect.element(update()).not.toBeInTheDocument();
});

test('a refused Update says why with its reference, and the icon stays', async () => {
  service(standing('ready', '9.9.9'));
  made.refuse = true;
  await render(<App />);
  await update().click();
  await expect.element(page.getByText('Update failed')).toBeVisible();
  await expect.element(page.getByRole('button', { name: 'Reference made-up-ref' })).toBeVisible();
  await expect.element(update()).toBeVisible();
});
