// The chat's tasks in one list (the tasks mock-up, New): For you, Open and Closed, each task a row with a plain
// face, its what, and one line "#24 · Bo · Blocked". Made-up team and tasks.
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Task } from '../api';
import { TaskList } from './TaskList';

const names: Record<string, string> = { mira: 'Mira', ann: 'Ann', bo: 'Bo', rev: 'Rev' };
const name = (agent: string | null) => (agent && names[agent]) || 'An agent';

const task = (
  number: number,
  owner: string | null,
  state: Task['state'],
  more: Partial<Task> = {},
): Task => ({
  number,
  chat: 'c1',
  giver: 'mira',
  owner,
  given_in: 't1',
  kind: 'work',
  what: `Made-up task ${number}`,
  state,
  reason: null,
  after: [],
  created_us: 0,
  ...more,
});
const question = (number: number, state: Task['state'], more: Partial<Task> = {}) =>
  task(number, null, state, { kind: 'person', ...more });

const tasks: Task[] = [
  question(31, 'ready'),
  question(33, 'ready', { giver: null }),
  task(27, 'ann', 'working'),
  task(28, 'rev', 'waiting', { after: [27] }),
  task(24, 'bo', 'blocked'),
  question(26, 'held'),
  task(22, 'ann', 'held', { pr: 18, waiting_on: ['#18 to merge'] }),
  task(29, 'bo', 'ready'),
  task(30, 'ann', 'working', { look_into: true }),
  task(25, 'ann', 'cancelled'),
  task(21, 'bo', 'done'),
  task(20, 'bo', 'failed'),
  task(18, 'ann', 'stopped'),
  question(17, 'done'),
];

test('the list groups For you, Open and Closed, each row one line of number, who and state', async () => {
  const opened: number[] = [];
  const screen = await render(<TaskList tasks={tasks} name={name} onOpen={(n) => opened.push(n)} />);
  const list = screen.container;
  const headings = [...list.querySelectorAll('h3')].map((h) => h.textContent);
  expect(headings).toEqual(['For you2', 'Open7', 'Closed5']);
  const lines = [...list.querySelectorAll('.task-row-line')].map((l) => l.textContent);
  expect(lines).toEqual([
    '#31·Mira·Waiting',
    '#33·Gathr·Waiting',
    '#22·Ann·Merging',
    '#24·Bo·Blocked',
    '#26·Mira·Discussing',
    '#27·Ann·Working',
    '#28·Rev·After #27',
    '#29·Bo·Not started',
    '#30·Ann·Look into·Working',
    '#25·Ann·Cancelled',
    '#21·Bo',
    '#20·Bo·Failed',
    '#18·Ann·Stopped',
    '#17·Mira',
  ]);
  // A state the marks draw takes the mark's shape, still; a row's face is plain.
  const working = list.querySelector('[data-task="27"] .task-row-line .mark');
  expect(working?.getAttribute('data-state')).toBe('working');
  expect(working?.hasAttribute('data-spin')).toBe(false);
  expect(list.querySelectorAll('.task-row .face .mark')).toHaveLength(0);
  // A done task's tick follows its what.
  expect(list.querySelector('[data-task="21"] .task-row-what svg')).not.toBeNull();
  await screen.getByRole('button', { name: /Made-up task 24/ }).click();
  expect(opened).toEqual([24]);
});
