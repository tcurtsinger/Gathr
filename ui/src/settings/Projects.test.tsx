// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import '../theme.css';
import type { Agent, Project } from '../api';
import type { LiveEvent } from '../live';
import { Projects } from './Projects';

vi.mock('../live', () => ({ listen: (_: (event: LiveEvent) => void) => () => {} }));

const agent = (id: string, name: string): Agent => ({
  id,
  name,
  account: 'a',
  model: null,
  effort: null,
  approvals: 'auto',
});
const set = '2026-11-08T12:00:00.000Z';
const project: Project = {
  id: 'p',
  name: 'Made-up project',
  folder: 'X:\\made-up\\repo',
  members: [
    { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
    { agent: 'b', name: 'Made-up Member', role: 'member' },
  ],
  checks: [
    { command: 'cargo test', source: 'found', set },
    { command: 'npm test', source: 'a', set },
  ],
  ci: 'gathr',
  merge: 'fast-forward',
};
const sent: { method: string; path: string; body: unknown }[] = [];

/** The service: its projects and agents, and an answer to each PATCH. */
function service(patch: (body: Record<string, unknown>) => Response) {
  const answer = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    const method = init?.method ?? 'GET';
    if (method === 'GET' && path === '/api/projects') return answer({ projects: [project] });
    if (method === 'GET' && path === '/api/agents')
      return answer({
        agents: [agent('a', 'Made-up Lead'), agent('b', 'Made-up Member'), agent('c', 'Made-up Third')],
      });
    if (method === 'PATCH' && path === '/api/projects/p') {
      const body = JSON.parse(String(init?.body));
      sent.push({ method, path, body });
      return patch(body);
    }
    return answer({});
  });
}
afterEach(() => {
  vi.unstubAllGlobals();
  sent.length = 0;
});

test('checks read with who set each and when, never as fields; an edit across both sections saves once', async () => {
  service((body) => new Response(JSON.stringify({ ...project, ...body, members: project.members })));
  const screen = await render(<Projects start="p" onReference={() => {}} />);

  await screen.getByRole('button', { name: 'Checks & merge' }).click();
  const day = new Date(set).toLocaleDateString([], { month: 'short', day: 'numeric' });
  const checks = screen.getByRole('list', { name: 'Checks' });
  await expect
    .element(checks)
    .toHaveTextContent(`cargo testFound in the project · ${day}npm testMade-up Lead · ${day}`);
  await expect.element(checks.getByRole('textbox')).not.toBeInTheDocument();
  await screen.getByRole('combobox', { name: 'Merge method' }).selectOptions('merge');

  await screen.getByRole('button', { name: 'General', exact: true }).click();
  await screen.getByRole('textbox', { name: 'Name' }).fill('Made-up renamed');
  await screen.getByRole('combobox', { name: 'Role of Made-up Member' }).selectOptions('reviewer');
  await screen.getByRole('combobox', { name: 'Add agent' }).selectOptions('c');
  await screen.getByRole('button', { name: 'Save' }).click();

  await expect.element(screen.getByRole('status')).toHaveTextContent('Saved');
  // Only what changed goes, the team whole as the service takes it; never the checks.
  expect(sent).toEqual([
    {
      method: 'PATCH',
      path: '/api/projects/p',
      body: {
        name: 'Made-up renamed',
        merge: 'merge',
        members: [
          { agent: 'a', role: 'supervisor' },
          { agent: 'b', role: 'reviewer' },
          { agent: 'c', role: 'member' },
        ],
      },
    },
  ]);
});

test('a refused save says why with its reference, and keeps the draft', async () => {
  const refused = { error: 'Project settings failed', why: 'made-up folder refusal', ref: 'made-up-ref' };
  service(() => new Response(JSON.stringify(refused), { status: 400 }));
  const opened: string[] = [];
  const screen = await render(<Projects start="p" onReference={(ref) => opened.push(ref)} />);

  await screen.getByRole('textbox', { name: 'Folder' }).fill('X:\\made-up\\elsewhere');
  await screen.getByRole('button', { name: 'Save' }).click();

  await expect.element(screen.getByText('made-up folder refusal')).toBeVisible();
  await screen.getByRole('button', { name: 'Reference made-up-ref' }).click();
  expect(opened).toEqual(['made-up-ref']);
  await expect.element(screen.getByRole('textbox', { name: 'Folder' })).toHaveValue('X:\\made-up\\elsewhere');
  expect(sent.map((s) => s.body)).toEqual([{ folder: 'X:\\made-up\\elsewhere' }]);
});

test('the team keeps one Supervisor and at least one agent; a new Supervisor or Reviewer makes the one before a Member', async () => {
  service((body) => new Response(JSON.stringify({ ...project, ...body, members: project.members })));
  const screen = await render(<Projects start="p" onReference={() => {}} />);
  const role = (name: string) => screen.getByRole('combobox', { name: `Role of ${name}` });
  const remove = (name: string) => screen.getByRole('button', { name: `Remove ${name}` });

  // The Supervisor isn't demoted directly: another is picked in its place.
  await expect.element(role('Made-up Lead')).toBeDisabled();
  await role('Made-up Member').selectOptions('supervisor');
  await expect.element(role('Made-up Lead')).toHaveValue('member');
  await expect.element(role('Made-up Member')).toBeDisabled();

  // One Reviewer: a second one makes the first a Member.
  await screen.getByRole('combobox', { name: 'Add agent' }).selectOptions('c');
  await role('Made-up Third').selectOptions('reviewer');
  await role('Made-up Lead').selectOptions('reviewer');
  await expect.element(role('Made-up Third')).toHaveValue('member');

  // Removing the Supervisor makes the next agent it; the last agent stays.
  await remove('Made-up Member').click();
  await expect.element(role('Made-up Lead')).toHaveValue('supervisor');
  await remove('Made-up Third').click();
  await expect.element(remove('Made-up Lead')).toBeDisabled();

  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.element(screen.getByRole('status')).toHaveTextContent('Saved');
  expect(sent.map((s) => s.body)).toEqual([{ members: [{ agent: 'a', role: 'supervisor' }] }]);
});

test("a check's setter keeps its name after leaving the team", async () => {
  const left = { ...project, checks: [{ command: 'npm test', source: 'c', set }] };
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/projects') return new Response(JSON.stringify({ projects: [left] }));
    if (path === '/api/agents')
      return new Response(JSON.stringify({ agents: [agent('c', 'Made-up Third')] }));
    return new Response('{}');
  });
  const screen = await render(<Projects start="p" onReference={() => {}} />);
  await screen.getByRole('button', { name: 'Checks & merge' }).click();
  await expect
    .element(screen.getByRole('list', { name: 'Checks' }))
    .toHaveTextContent(
      `npm testMade-up Third · ${new Date(set).toLocaleDateString([], { month: 'short', day: 'numeric' })}`,
    );
});
