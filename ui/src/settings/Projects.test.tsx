// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Corner } from '../components/Corner';
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
  shape: null,
  eyes: null,
  colour: null,
});
const start: Project = {
  id: 'p',
  name: 'Made-up project',
  folder: 'X:\\made-up\\repo',
  members: [
    { agent: 'a', name: 'Made-up Lead', role: 'supervisor' },
    { agent: 'b', name: 'Made-up Member', role: 'member' },
  ],
};
let project = { ...start };
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
  project = { ...start };
});

test('a refused save says why with its reference, and puts the folder back', async () => {
  const refused = { error: 'Save failed', why: 'made-up folder refusal', ref: 'made-up-ref' };
  service(() => new Response(JSON.stringify(refused), { status: 400 }));
  const opened: string[] = [];
  const screen = await render(
    <>
      <Projects start="p" onAdd={() => {}} />
      <Corner onReference={(ref) => opened.push(ref)} />
    </>,
  );

  await screen.getByRole('textbox', { name: 'Folder' }).fill('X:\\made-up\\elsewhere');
  await userEvent.keyboard('{Enter}');

  await expect.element(screen.getByText('made-up folder refusal')).toBeVisible();
  await screen.getByRole('button', { name: 'Reference made-up-ref' }).click();
  expect(opened).toEqual(['made-up-ref']);
  await expect.element(screen.getByRole('textbox', { name: 'Folder' })).toHaveValue('X:\\made-up\\repo');
  expect(sent.map((s) => s.body)).toEqual([{ folder: 'X:\\made-up\\elsewhere' }]);
});

test('each team change saves at once; removing the lead makes the next one lead, and the last agent stays', async () => {
  const names: Record<string, string> = { a: 'Made-up Lead', b: 'Made-up Member', c: 'Made-up Third' };
  service((body) => {
    const members = (body.members as { agent: string; role: string }[]).map((m) => ({
      ...m,
      name: names[m.agent],
    }));
    project.members = members as Project['members'];
    return new Response(JSON.stringify({ ...project, members }));
  });
  const screen = await render(<Projects start="p" onAdd={() => {}} />);
  const lead = screen.getByRole('combobox', { name: 'Lead' });
  const remove = (name: string) => screen.getByRole('button', { name: `Remove ${name}` });
  const pick = async (field: ReturnType<typeof screen.getByRole>, choice: string) => {
    await field.click();
    await page.getByRole('option', { name: choice, exact: true }).click();
  };

  await expect.element(lead).toHaveTextContent('Made-up Lead');
  await pick(lead, 'Made-up Member');
  await expect.element(lead).toHaveTextContent('Made-up Member');
  await expect.element(screen.getByRole('status')).toHaveTextContent('Saved');

  await remove('Made-up Member').click();
  await expect.element(lead).toHaveTextContent('Made-up Lead');
  await expect.element(remove('Made-up Lead')).toBeDisabled();
  expect(sent.map((s) => s.body)).toEqual([
    {
      members: [
        { agent: 'a', role: 'member' },
        { agent: 'b', role: 'supervisor' },
      ],
    },
    { members: [{ agent: 'a', role: 'supervisor' }] },
  ]);
});

test('Delete asks in place with what the service counts, then deletes the project and shows the list', async () => {
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
  const screen = await render(<Projects start="p" onAdd={() => {}} />);
  await screen.getByRole('button', { name: 'Delete' }).click();
  const confirm = screen.getByRole('group').filter({ hasText: 'Delete Made-up project?' });
  await expect.element(confirm.getByText('4 chats will be deleted. Files are kept.')).toBeVisible();
  await confirm.getByRole('button', { name: 'Delete' }).click();

  await expect.element(screen.getByText('Made-up other')).toBeVisible();
  await expect.element(screen.getByText('Made-up project')).not.toBeInTheDocument();
  expect(deleted).toEqual(['/api/projects/p']);
});
