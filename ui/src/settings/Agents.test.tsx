// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
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
      return answer({ id: 'new', face_seed: made.name, gender: null, skin: null, background: null, ...made });
    }
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
  face_seed: 'Made-up agent',
  gender: null,
  skin: null,
  background: null,
  ...over,
});

test('only exact models are listed, in the engine’s order; a model or effort no longer listed says so', async () => {
  service([agent({ model: 'made-up-1-0', effort: 'made-up-effort' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const list = screen.getByRole('group', { name: 'Models' }).getByRole('button');
  await expect.element(list.first()).toHaveTextContent('Made-up 9.9');
  expect(list.elements().map((b) => b.textContent)).toEqual(['Made-up 9.9', 'Opus 5.5', 'Sonnet 5.5']);
  await expect.element(screen.getByText('made-up-1-0 / Unavailable')).toBeVisible();
  await expect.element(screen.getByRole('combobox', { name: 'Effort' })).toHaveValue('made-up-effort');
});

test('an agent saved on a Latest or the engine default shows the model that is today, and picking it keeps it', async () => {
  const agents = [agent({ model: 'opus' })];
  service(agents, listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const opus = screen.getByRole('button', { name: 'Opus 5.5' });
  await expect.element(opus).toHaveAttribute('aria-pressed', 'true');
  await expect.element(screen.getByText('Opus 5.5 / Latest')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

  agents[0] = agent({ model: null });
  savedElsewhere();
  await expect.element(screen.getByText('Opus 5.5 / Engine default')).toBeVisible();
  await expect.element(opus).toHaveAttribute('aria-pressed', 'true');

  await opus.click();
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.poll(() => sent.at(-1)?.body).toMatchObject({ model: 'made-up-opus-5-5' });
});

test('another account takes that account’s default model, resets effort and lists its own models', async () => {
  service([agent({ model: 'made-up-opus-5-5', effort: 'high' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  await expect
    .element(screen.getByRole('button', { name: 'Opus 5.5' }))
    .toHaveAttribute('aria-pressed', 'true');
  await screen.getByRole('combobox', { name: 'Account' }).selectOptions('b');
  await expect
    .element(screen.getByRole('button', { name: 'Haiku 5.5' }))
    .toHaveAttribute('aria-pressed', 'true');
  await expect.element(screen.getByRole('combobox', { name: 'Effort' })).toHaveValue('');
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
  await expect
    .element(screen.getByRole('button', { name: 'Opus 5.5' }))
    .toHaveAttribute('aria-pressed', 'true');

  agents[0] = agent({ model: 'made-up-sonnet-5-5', effort: 'high' });
  savedElsewhere();
  await expect
    .element(screen.getByRole('button', { name: 'Sonnet 5.5' }))
    .toHaveAttribute('aria-pressed', 'true');
  await expect.element(effort).toHaveValue('high');
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

  await effort.selectOptions('low');
  agents[0] = agent({ model: 'made-up-opus-5-5', effort: 'high' });
  savedElsewhere();
  await expect
    .element(screen.getByRole('button', { name: 'Opus 5.5' }))
    .toHaveAttribute('aria-pressed', 'true');
  await expect.element(effort).toHaveValue('low');
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
  await expect
    .element(screen.getByRole('button', { name: 'Opus 5.5' }))
    .toHaveAttribute('aria-pressed', 'true');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.element(screen.getByText('Saved')).toBeVisible();
  expect(sent).toEqual([
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
  await expect.element(approvals).toHaveValue('full');
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  await approvals.selectOptions('Ask');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.poll(() => sent.at(-1)?.body).toMatchObject({ approvals: 'ask' });
});

test('Codex lists its exact models without Gathr’s Latest rows, and a new agent takes the default’s', async () => {
  const models = [
    { ...model('default', 'Engine default'), resolved: 'gpt-6.1-sol' },
    { ...model('latest:astra', 'Astra Latest'), latest: true, resolved: 'gpt-6-astra' },
    { ...model('latest:sol', 'Sol Latest'), latest: true, resolved: 'gpt-6.1-sol' },
    { ...model('gpt-6-astra', 'GPT-6-Astra'), resolved: 'gpt-6-astra' },
    { ...model('gpt-6.1-sol', 'GPT-6.1-Sol'), resolved: 'gpt-6.1-sol' },
  ];
  service([], () => new Response(JSON.stringify({ models })));
  const screen = await render(<Agents onReference={() => {}} />);
  await screen.getByRole('button', { name: 'Add', exact: true }).click();
  await expect
    .element(screen.getByRole('button', { name: 'GPT-6.1-Sol', exact: true }))
    .toHaveAttribute('aria-pressed', 'true');
  const list = screen.getByRole('group', { name: 'Models' }).getByRole('button');
  expect(list.elements().map((b) => b.textContent)).toEqual(['GPT-6-Astra', 'GPT-6.1-Sol']);
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

test('a face never set shows what its seed picked; a gender or a tone chosen redraws it and is saved', async () => {
  // Ann's seed picks a woman of tone 5 (the faces mock-up's generator).
  service([agent({ name: 'Ann', face_seed: 'Ann' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const face = screen.getByRole('img', { name: 'Ann' });
  const gender = screen.getByRole('combobox', { name: 'Gender' });
  await expect.element(gender).toHaveValue('woman');
  await expect
    .element(screen.getByRole('button', { name: 'Skin tone 5' }))
    .toHaveAttribute('aria-pressed', 'true');
  await expect.element(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

  const before = face.element().innerHTML;
  await gender.selectOptions('Man');
  expect(face.element().innerHTML).not.toBe(before);
  const man = face.element().innerHTML;
  await screen.getByRole('button', { name: 'Skin tone 2' }).click();
  await expect
    .element(screen.getByRole('button', { name: 'Skin tone 2' }))
    .toHaveAttribute('aria-pressed', 'true');
  expect(face.element().innerHTML).not.toBe(man);

  await screen.getByRole('button', { name: 'Save' }).click();
  await expect
    .poll(() => sent.at(-1)?.body)
    .toEqual({
      name: 'Ann',
      account: 'a',
      model: null,
      effort: null,
      approvals: 'auto',
      gender: 'man',
      skin: 2,
    });
});

test('Shuffle draws a new face from a new seed and keeps the gender and tone shown, saving them', async () => {
  // Nothing set: Ann's seed picks a woman of tone 5, and the form shows them.
  service([agent({ name: 'Ann', face_seed: 'Ann' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const face = screen.getByRole('img', { name: 'Ann' });
  const gender = screen.getByRole('combobox', { name: 'Gender' });
  await expect.element(gender).toHaveValue('woman');
  const before = face.element().innerHTML;
  await screen.getByRole('button', { name: 'Shuffle' }).click();
  expect(face.element().innerHTML).not.toBe(before);
  await expect.element(gender).toHaveValue('woman');
  await expect
    .element(screen.getByRole('button', { name: 'Skin tone 5' }))
    .toHaveAttribute('aria-pressed', 'true');

  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.poll(() => sent.length).toBe(1);
  const { face_seed, ...rest } = sent[0].body as Record<string, unknown>;
  expect([typeof face_seed, face_seed === 'Ann', rest]).toEqual([
    'string',
    false,
    { name: 'Ann', account: 'a', model: null, effort: null, approvals: 'auto', gender: 'woman', skin: 5 },
  ]);
});

/** The colour a face is drawn on, as the browser computed it. */
const drawnOn = (face: { element: () => Element }) => getComputedStyle(face.element()).backgroundColor;

test('a face with no background is grey; a ready colour pressed redraws it, Save sends it and Cancel drops it', async () => {
  service([agent({ name: 'Ann', face_seed: 'Ann' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const face = screen.getByRole('img', { name: 'Ann' });
  await expect.poll(() => drawnOn(face)).toBe('rgb(206, 206, 206)');
  const mint = screen.getByRole('button', { name: 'Mint' });
  await expect.element(mint).toHaveAttribute('aria-pressed', 'false');

  await mint.click();
  await expect.element(mint).toHaveAttribute('aria-pressed', 'true');
  expect(drawnOn(face)).toBe('rgb(108, 213, 179)');
  await screen.getByRole('button', { name: 'Cancel' }).click();
  await expect.element(mint).toHaveAttribute('aria-pressed', 'false');
  expect(drawnOn(face)).toBe('rgb(206, 206, 206)');

  await screen.getByRole('button', { name: 'Rose' }).click();
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect
    .poll(() => sent.at(-1)?.body)
    .toEqual({
      name: 'Ann',
      account: 'a',
      model: null,
      effort: null,
      approvals: 'auto',
      background: '#FCA0A7',
    });
});

test('the + opens the picker under the row; a colour picked there shows on the face and the + and is saved', async () => {
  service([agent({ name: 'Ann', face_seed: 'Ann', background: '#90D192' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const face = screen.getByRole('img', { name: 'Ann' });
  const custom = screen.getByRole('button', { name: 'Custom' });
  await expect.element(screen.getByRole('button', { name: 'Leaf' })).toHaveAttribute('aria-pressed', 'true');
  await expect.element(custom).toHaveAttribute('aria-pressed', 'false');

  await custom.click();
  await expect.element(custom).toHaveAttribute('aria-expanded', 'true');
  const hex = screen.getByRole('textbox', { name: 'Hex' });
  await expect.element(hex).toHaveValue('90D192');
  await hex.fill('7fb5a8');
  await expect.element(custom).toHaveAttribute('aria-pressed', 'true');
  await expect.element(screen.getByRole('button', { name: 'Leaf' })).toHaveAttribute('aria-pressed', 'false');
  expect(drawnOn(face)).toBe('rgb(127, 181, 168)');

  screen.getByRole('slider', { name: 'Hue' }).element().focus();
  await userEvent.keyboard('{Escape}');
  await expect.element(custom).toHaveAttribute('aria-expanded', 'false');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.poll(() => sent.at(-1)?.body).toMatchObject({ background: '#7FB5A8' });
});

test('a saved colour that is not one of the twelve shows pressed on the +', async () => {
  service([agent({ name: 'Ann', face_seed: 'Ann', background: '#7FB5A8' })], listed);
  const screen = await render(<Agents onReference={() => {}} />);
  const custom = screen.getByRole('button', { name: 'Custom' });
  await expect.element(custom).toHaveAttribute('aria-pressed', 'true');
  expect(getComputedStyle(custom.element()).backgroundColor).toBe('rgb(127, 181, 168)');
  const pressed = screen.getByRole('group', { name: 'Background' }).getByRole('button', { pressed: true });
  expect(pressed.elements().map((b) => b.getAttribute('aria-label'))).toEqual(['Custom']);
  await expect.poll(() => drawnOn(screen.getByRole('img', { name: 'Ann' }))).toBe('rgb(127, 181, 168)');
});
