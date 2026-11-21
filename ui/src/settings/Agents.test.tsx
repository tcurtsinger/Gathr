// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
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

/** A family row and the exact model it means today, as the service lists them. */
const family = (id: string, exact: string, name: string, efforts?: string[]): Model[] => [
  { ...model(id, name, efforts), resolved: exact },
  { ...model(exact, name, efforts), resolved: exact },
];

/** Made-up lists as the service gives them: default first, families with their exact model, older models. */
const lists: Record<string, Model[]> = {
  a: [
    { ...model('default', 'Default'), resolved: 'made-up-opus-5-5' },
    model('made-up-9-9', 'Made-up 9.9'),
    ...family('opus', 'made-up-opus-5-5', 'Opus 5.5'),
    ...family('sonnet', 'made-up-sonnet-5-5', 'Sonnet 5.5'),
  ],
  b: [
    { ...model('default', 'Default', ['medium']), resolved: 'made-up-haiku-5-5' },
    ...family('haiku', 'made-up-haiku-5-5', 'Haiku 5.5', ['medium']),
  ],
};
const sent: { method: string; path: string; body: unknown }[] = [];

function service(agents: Agent[], models: (account: string) => Response | Promise<Response>) {
  const answer = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    const method = init?.method ?? 'GET';
    if (method !== 'GET' && path !== '/api/ui/events')
      sent.push({ method, path, body: init?.body === undefined ? undefined : JSON.parse(String(init.body)) });
    if (path === '/api/agents' && method === 'GET') return answer({ agents, removed: [] });
    // Mira leads where Ann sits, with a task open.
    const counts = { chats: 0, tasks: 1, projects: 0, leads: ['Ann'] };
    if (path.endsWith('/delete')) return answer(counts);
    if (method === 'DELETE') return answer({ ...counts, deleted: true, left: [] });
    if (path === '/api/agents') {
      const made = JSON.parse(String(init?.body));
      return answer({ id: 'new', shape: null, eyes: null, colour: null, ...made });
    }
    if (path === '/api/accounts')
      return answer({ accounts: [account('a', 'Made-up one'), account('b', 'Made-up two')] });
    const asked = path.match(/^\/api\/accounts\/(\w+)\/models$/);
    if (asked) return models(asked[1]);
    return answer({});
  });
}
/** Picks a choice from one of the form's dropdowns, as a person does. */
const pick = async (field: string, choice: string) => {
  await page.getByRole('combobox', { name: field }).click();
  await page.getByRole('option', { name: choice, exact: true }).click();
};
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
  shape: null,
  eyes: null,
  colour: null,
  ...over,
});

test('an agent saved on a Latest or the engine default shows the model that is today, and picking it keeps it', async () => {
  const agents = [agent({ model: 'opus' })];
  service(agents, listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const model = screen.getByRole('combobox', { name: 'Model' });
  await expect.element(model).toHaveTextContent('Opus 5.5 / Latest');
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

  agents[0] = agent({ model: null });
  savedElsewhere();
  await expect.element(model).toHaveTextContent('Opus 5.5 / Engine default');

  await pick('Model', 'Opus 5.5');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.poll(() => sent.at(-1)?.body).toMatchObject({ model: 'made-up-opus-5-5' });
});

test('another account takes that account’s default model, resets effort and lists its own models', async () => {
  service([agent({ model: 'made-up-opus-5-5', effort: 'high' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const model = screen.getByRole('combobox', { name: 'Model' });
  await expect.element(model).toHaveTextContent('Opus 5.5');
  await pick('Account', 'Made-up two / Claude Code');
  await expect.element(model).toHaveTextContent('Haiku 5.5');
  await expect.element(screen.getByRole('combobox', { name: 'Effort' })).toHaveTextContent('Engine default');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect
    .poll(() => sent)
    .toEqual([
      {
        method: 'PATCH',
        path: '/api/agents/x',
        body: {
          name: 'Made-up agent',
          account: 'b',
          model: 'made-up-haiku-5-5',
          effort: null,
          approvals: 'auto',
        },
      },
    ]);
});

test('a change saved elsewhere shows in the open editor, and Save keeps it unless the field was edited here', async () => {
  const agents = [agent({ model: 'made-up-opus-5-5', effort: null })];
  service(agents, listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const effort = screen.getByRole('combobox', { name: 'Effort' });
  const model = screen.getByRole('combobox', { name: 'Model' });
  await expect.element(model).toHaveTextContent('Opus 5.5');

  agents[0] = agent({ model: 'made-up-sonnet-5-5', effort: 'high' });
  savedElsewhere();
  await expect.element(model).toHaveTextContent('Sonnet 5.5');
  await expect.element(effort).toHaveTextContent('high');
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

  await pick('Effort', 'low');
  agents[0] = agent({ model: 'made-up-opus-5-5', effort: 'high' });
  savedElsewhere();
  await expect.element(model).toHaveTextContent('Opus 5.5');
  await expect.element(effort).toHaveTextContent('low');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect
    .poll(() => sent.at(-1)?.body)
    .toEqual({
      name: 'Made-up agent',
      account: 'a',
      model: 'made-up-opus-5-5',
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

test('a new agent takes the exact model its account’s engine default means and is saved with it', async () => {
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
  await expect.element(screen.getByRole('combobox', { name: 'Model' })).toHaveTextContent('Opus 5.5');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.element(screen.getByText('Saved')).toBeVisible();
  expect(sent).toMatchObject([
    {
      method: 'POST',
      path: '/api/agents',
      body: { name: 'Made-up new', account: 'a', model: 'made-up-opus-5-5', effort: null, approvals: 'auto' },
    },
  ]);
});

test('an account that lists no exact model can’t be saved for a new agent', async () => {
  const none = [{ ...model('default', 'Default'), resolved: null }, model('opus', 'Opus')];
  service([], () => new Response(JSON.stringify({ models: none })));
  const screen = await render(<Agents onReference={() => {}} />);
  await screen.getByRole('button', { name: 'Add', exact: true }).click();
  await screen.getByRole('textbox', { name: 'Name' }).fill('Made-up new');
  await expect.element(screen.getByText('No models')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
});

test('a change of approvals enables Save and is saved', async () => {
  service([agent({ approvals: 'full' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const approvals = screen.getByRole('combobox', { name: 'Approvals' });
  await expect.element(approvals).toHaveTextContent('Full');
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  await pick('Approvals', 'Ask');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.poll(() => sent.at(-1)?.body).toMatchObject({ approvals: 'ask' });
});

test('Delete asks with what the service counts, then deletes the agent', async () => {
  service([agent({ name: 'Mira' }), agent({ id: 'y', name: 'Ann' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  await screen.getByRole('button', { name: 'Delete' }).click();
  const confirm = screen.getByRole('dialog', { name: 'Delete Mira?' });
  await expect.element(confirm.getByText('1 task will be cancelled. Ann becomes lead.')).toBeVisible();
  await confirm.getByRole('button', { name: 'Delete' }).click();

  await expect.element(confirm).not.toBeInTheDocument();
  await expect.element(screen.getByRole('button', { name: 'Mira' })).not.toBeInTheDocument();
  expect(sent).toEqual([{ method: 'DELETE', path: '/api/agents/x', body: undefined }]);
});
