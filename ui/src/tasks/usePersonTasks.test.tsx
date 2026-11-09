// The service is the edge here: fetch answers as it would, and live events arrive when the test says.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { PersonTask } from '../api';
import type { LiveEvent } from '../live';
import { usePersonTasks } from './usePersonTasks';

const live = vi.hoisted(() => new Set<(event: LiveEvent) => void>());
vi.mock('../live', () => ({
  listen: (listener: (event: LiveEvent) => void) => {
    live.add(listener);
    return () => live.delete(listener);
  },
}));
afterEach(() => vi.unstubAllGlobals());

const ask = (number: number) =>
  ({ number, chat: 'c1', form: 'yesno', choices: [], files: [] }) as unknown as PersonTask;
const json = (body: unknown) => new Response(JSON.stringify(body));
const noFailure = () => {};

let person: ReturnType<typeof usePersonTasks>;
function Probe() {
  person = usePersonTasks(noFailure);
  const staged = Object.keys(person.drafts).join(',');
  return <p>{`open ${person.tasks.map((t) => t.number).join(',')} drafts ${staged}`}</p>;
}

test('a Send the service takes closes its questions even with no live event', async () => {
  vi.stubGlobal('fetch', async (path: string) =>
    // The service answers each question it took whole, as a task.
    path === '/api/person-tasks'
      ? json({ tasks: [ask(1), ask(2)], stale: [] })
      : json({ answered: [ask(1)], refused: [] }),
  );
  const screen = await render(<Probe />);
  await expect.element(screen.getByText('open 1,2 drafts')).toBeVisible();
  person.draft(1, { values: ['Yes'], notes: '' });
  person.draft(2, { values: ['No'], notes: '' });
  await expect.element(screen.getByText('open 1,2 drafts 1,2')).toBeVisible();
  await person.send([ask(1)]);
  await expect.element(screen.getByText('open 2 drafts 2')).toBeVisible();
});

test('a list read that answers after a live update never brings back what the update closed', async () => {
  let release = (_: Response) => {};
  vi.stubGlobal('fetch', () => new Promise<Response>((done) => (release = done)));
  live.clear();
  const screen = await render(<Probe />);
  await expect.poll(() => live.size).toBe(1);
  for (const listener of live) listener({ type: 'person_tasks', tasks: [ask(2)], stale: [] });
  await expect.element(screen.getByText('open 2 drafts')).toBeVisible();
  release(json({ tasks: [ask(1), ask(2)], stale: [] }));
  await new Promise((r) => setTimeout(r, 50));
  await expect.element(screen.getByText('open 2 drafts')).toBeVisible();
});
