// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Account, Agent, Model } from '../api';
import type { LiveEvent } from '../live';
import { Agents } from './Agents';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('../live', () => ({
  listen: (listener: (event: LiveEvent) => void) => {
    live.add(listener);
    return () => live.delete(listener);
  },
}));
const savedElsewhere = () => {
  for (const listener of live) listener({ type: 'agents' });
};

const account = (id: string, name: string): Account => ({
  id,
  name,
  engine: 'claude',
  login: 'signed_in',
  expires: null,
  sign_in: null,
});
const model = (id: string, name: string, efforts = ['low', 'high']): Model => ({
  id,
  name,
  latest: ['opus', 'sonnet', 'haiku'].includes(id),
  description: null,
  efforts,
});

/** Made-up lists as Claude Code gives them: default first, aliases, then a numbered model. */
const lists: Record<string, Model[]> = {
  a: [
    model('default', 'Default'),
    model('made-up-9-9', 'Made-up 9.9'),
    model('opus', 'Opus 5.5'),
    model('sonnet', 'Sonnet'),
  ],
  b: [model('default', 'Default', ['medium']), model('haiku', 'Haiku', ['medium'])],
};
const sent: { method: string; path: string; body: unknown }[] = [];

function service(agents: Agent[], models: (account: string) => Response | Promise<Response>) {
  const answer = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    const method = init?.method ?? 'GET';
    if (method !== 'GET' && path !== '/api/ui/events')
      sent.push({ method, path, body: JSON.parse(String(init?.body)) });
    if (path === '/api/agents' && method === 'GET') return answer({ agents });
    if (path === '/api/agents') return answer({ id: 'new', ...JSON.parse(String(init?.body)) });
    if (path === '/api/accounts')
      return answer({ accounts: [account('a', 'Made-up one'), account('b', 'Made-up two')] });
    const asked = path.match(/^\/api\/accounts\/(\w+)\/models$/);
    if (asked) return models(asked[1]);
    return answer({});
  });
}
const listed = (id: string) => new Response(JSON.stringify({ models: lists[id] }));
afterEach(() => {
  vi.unstubAllGlobals();
  sent.length = 0;
});

const agent = (over: Partial<Agent>): Agent => ({
  id: 'x',
  name: 'Made-up agent',
  account: 'a',
  model: null,
  effort: null,
  approvals: 'auto',
  ...over,
});

test('Latest comes first, then Engine default, then numbered models; a model or effort no longer listed says so', async () => {
  service([agent({ model: 'made-up-1-0', effort: 'made-up-effort' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const list = screen.getByRole('group', { name: 'Models' }).getByRole('button');
  await expect.element(list.first()).toHaveTextContent('Opus Latest');
  expect(list.elements().map((b) => b.textContent)).toEqual([
    'Opus Latest',
    'Sonnet Latest',
    'Engine default',
    'Made-up 9.9',
  ]);
  await expect.element(screen.getByText('made-up-1-0 / Unavailable')).toBeVisible();
  await expect.element(screen.getByRole('combobox', { name: 'Effort' })).toHaveValue('made-up-effort');
});

test('another account resets model and effort to the engine default and lists its own models', async () => {
  service([agent({ model: 'opus', effort: 'high' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  await expect
    .element(screen.getByRole('button', { name: 'Opus Latest' }))
    .toHaveAttribute('aria-pressed', 'true');
  await screen.getByRole('combobox', { name: 'Account' }).selectOptions('b');
  await expect.element(screen.getByRole('button', { name: 'Haiku Latest' })).toBeVisible();
  await expect
    .element(screen.getByRole('button', { name: 'Engine default' }))
    .toHaveAttribute('aria-pressed', 'true');
  await expect.element(screen.getByRole('combobox', { name: 'Effort' })).toHaveValue('');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect
    .poll(() => sent)
    .toEqual([
      {
        method: 'PATCH',
        path: '/api/agents/x',
        body: { name: 'Made-up agent', account: 'b', model: null, effort: null, approvals: 'auto' },
      },
    ]);
});

test('a change saved elsewhere shows in the open editor, and Save keeps it unless the field was edited here', async () => {
  const agents = [agent({ model: 'opus', effort: null })];
  service(agents, listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const effort = screen.getByRole('combobox', { name: 'Effort' });
  await expect
    .element(screen.getByRole('button', { name: 'Opus Latest' }))
    .toHaveAttribute('aria-pressed', 'true');

  agents[0] = agent({ model: 'sonnet', effort: 'high' });
  savedElsewhere();
  await expect
    .element(screen.getByRole('button', { name: 'Sonnet Latest' }))
    .toHaveAttribute('aria-pressed', 'true');
  await expect.element(effort).toHaveValue('high');
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

  await effort.selectOptions('low');
  agents[0] = agent({ model: 'opus', effort: 'high' });
  savedElsewhere();
  await expect
    .element(screen.getByRole('button', { name: 'Opus Latest' }))
    .toHaveAttribute('aria-pressed', 'true');
  await expect.element(effort).toHaveValue('low');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect
    .poll(() => sent.at(-1)?.body)
    .toEqual({
      name: 'Made-up agent',
      account: 'a',
      model: 'opus',
      effort: 'low',
      approvals: 'auto',
    });
});

test('models that fail to load say so with a reference that opens', async () => {
  const failed = { error: 'Models failed', why: 'made-up engine failure', ref: 'made-up-ref' };
  service([agent({})], () => new Response(JSON.stringify(failed), { status: 500 }));
  const opened: string[] = [];
  const screen = await render(<Agents onReference={(ref) => opened.push(ref)} />);
  await expect.element(screen.getByText('Models failed')).toBeVisible();
  await screen.getByRole('button', { name: 'Reference made-up-ref' }).click();
  expect(opened).toEqual(['made-up-ref']);
});

test('a new agent takes its account’s first Latest and is saved with it', async () => {
  let release = () => {};
  const slow = new Promise<void>((r) => {
    release = r;
  });
  service([], (id) => slow.then(() => listed(id)));
  const screen = await render(<Agents onReference={() => {}} />);
  await screen.getByRole('button', { name: 'Add' }).click();
  await screen.getByRole('textbox', { name: 'Name' }).fill('Made-up new');
  await expect.element(screen.getByText('Loading models')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  release();
  await expect
    .element(screen.getByRole('button', { name: 'Opus Latest' }))
    .toHaveAttribute('aria-pressed', 'true');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.element(screen.getByText('Saved')).toBeVisible();
  expect(sent).toEqual([
    {
      method: 'POST',
      path: '/api/agents',
      body: { name: 'Made-up new', account: 'a', model: 'opus', effort: null, approvals: 'auto' },
    },
  ]);
});

test('a change of approvals enables Save and is saved', async () => {
  service([agent({ approvals: 'full' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const approvals = screen.getByRole('combobox', { name: 'Approvals' });
  await expect.element(approvals).toHaveValue('full');
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  await approvals.selectOptions('Ask');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.poll(() => sent.at(-1)?.body).toMatchObject({ approvals: 'ask' });
});
