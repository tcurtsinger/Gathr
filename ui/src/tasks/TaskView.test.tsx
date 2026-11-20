// A task's view offers only what acts on it (the tasks mock-up, New): done has no foot, a waiting task no Restart.
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Task } from '../api';
import { TaskView } from './TaskView';

const task = (state: Task['state']): Task => ({
  number: 28,
  chat: 'c1',
  giver: 'mira',
  owner: 'rev',
  given_in: 't1',
  kind: 'work',
  what: 'Review the fix',
  state,
  reason: null,
  after: [],
  created_us: 0,
});

const offered: [Task['state'], string[]][] = [
  ['done', []],
  ['waiting', ['Cancel']],
  ['ready', ['Cancel']],
  ['working', ['Cancel']],
  ['held', ['Cancel']],
  ['blocked', ['Cancel', 'Restart']],
  ['failed', ['Cancel', 'Restart']],
  ['stopped', ['Cancel', 'Restart']],
  ['cancelled', ['Restart']],
];

test.each(offered)('a %s task offers %j', async (state, buttons) => {
  const screen = await render(
    <TaskView
      task={task(state)}
      tasks={[]}
      name={(a) => a ?? 'An agent'}
      onSettle={async () => {}}
      onReference={() => {}}
      onBack={() => {}}
    />,
  );
  await expect.element(screen.getByRole('heading', { name: 'Review the fix' })).toBeVisible();
  const foot = screen.container.querySelector('footer');
  expect([...(foot?.querySelectorAll('button') ?? [])].map((b) => b.textContent)).toEqual(buttons);
});

// The owner, 2026-11-19: "We need the same formatting that is in the chat in the tasks / plans".
test("a task's why, brief and reason are formatted as an agent's words in the chat are", async () => {
  const shown: Task = {
    ...task('blocked'),
    why: 'The filter reads **every** note.',
    done: '- `cargo nextest run` passes\n- the list filters',
    reason: 'Waiting on `#27`.',
  };
  const screen = await render(
    <TaskView
      task={shown}
      tasks={[]}
      name={(a) => a ?? 'An agent'}
      onSettle={async () => {}}
      onReference={() => {}}
      onBack={() => {}}
    />,
  );
  await expect.element(screen.getByText('every', { exact: true })).toBeVisible();
  const fields = screen.container.querySelector('dl.task-fields') as HTMLElement;
  expect(fields.textContent).not.toContain('**');
  expect(fields.textContent).not.toContain('`');
  expect(fields.querySelector('[data-streamdown="strong"]')?.textContent).toBe('every');
  expect([...fields.querySelectorAll('[data-streamdown="inline-code"]')].map((c) => c.textContent)).toEqual([
    '#27',
    'cargo nextest run',
  ]);
  expect(fields.querySelectorAll('[data-streamdown="list-item"]').length).toBe(2);
});
