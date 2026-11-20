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
  face_seed: 'Made-up',
  gender: null,
  skin: null,
  background: null,
});
const project: Project = {
  id: 'p',
  name: 'Made-up project',
  folder: 'X:\\made-up\\repo',
  members: [
    { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
    { agent: 'b', name: 'Made-up Member', role: 'member' },
  ],
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
        removed: [],
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

test('Delete asks with what the service counts, then deletes the project and shows the next', async () => {
  let projects = [project, { ...project, id: 'q', name: 'Made-up other' }];
  const deleted: string[] = [];
  const answer = (body: unknown) => new Response(JSON.stringify(body));
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/projects') return answer({ projects });
    if (path === '/api/agents') return answer({ agents: [], removed: [] });
    if (path === '/api/projects/p/delete') return answer({ chats: 4, tasks: 0, projects: 1, leads: [] });
    if (path === '/api/projects/p' && init?.method === 'DELETE') {
      deleted.push(path);
      projects = projects.slice(1);
      return answer({ chats: 4, tasks: 0, projects: 1, leads: [], deleted: true, left: [] });
    }
    return answer({});
  });
  const screen = await render(<Projects start="p" onReference={() => {}} />);
  await screen.getByRole('button', { name: 'Delete' }).click();
  const confirm = screen.getByRole('dialog', { name: 'Delete Made-up project?' });
  await expect.element(confirm.getByText('4 chats will be deleted. Files are kept.')).toBeVisible();
  await confirm.getByRole('button', { name: 'Delete' }).click();

  await expect.element(confirm).not.toBeInTheDocument();
  await expect.element(screen.getByRole('heading', { name: 'Made-up other' })).toBeVisible();
  expect(deleted).toEqual(['/api/projects/p']);
});
