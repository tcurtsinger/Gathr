// The chat's Tasks tab: the list, and a task opened from it or from the chat's Open task (the tasks mock-up, New).
import { useState } from 'react';
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Task } from '../api';
import type { Draft } from './model';
import { TasksTab } from './TasksTab';

const question: Task = {
  number: 3,
  chat: 'c1',
  giver: 'mira',
  owner: null,
  given_in: 't1',
  kind: 'person',
  what: 'Turn import on for everyone now, or behind a setting first?',
  why: 'PR #18 is ready to land. One thing is yours to decide first.',
  state: 'ready',
  reason: null,
  after: [],
  form: 'choice',
  choices: [{ label: 'On for everyone' }, { label: 'Behind a setting' }],
  files: [],
  created_us: 0,
};
const work: Task = {
  ...question,
  number: 82,
  kind: 'work',
  owner: 'ann',
  what: 'Fix it',
  why: null,
  state: 'working',
};

/** The tab as the app holds it: the opened task and the drafts live outside it. */
function Tab({ first }: { first?: number }) {
  const [opened, setOpened] = useState(first);
  const [drafts, setDrafts] = useState<Record<number, Draft>>({});
  return (
    <TasksTab
      tasks={[question, work]}
      members={[{ agent: 'mira', name: 'Mira', role: 'supervisor' }]}
      opened={opened}
      onOpen={setOpened}
      drafts={drafts}
      onDraft={(n, d) => setDrafts((all) => ({ ...all, [n]: d }))}
      name={(a) => (a === 'mira' ? 'Mira' : a === 'ann' ? 'Ann' : 'An agent')}
      onOpenFile={() => {}}
      onAnswer={async () => null}
      onSettle={async () => {}}
      onReference={() => {}}
    />
  );
}

test('Open task on a question opens the question itself, answered in place; Back leads to the list', async () => {
  const screen = await render(<Tab first={3} />);
  await expect.element(screen.getByText(question.why as string)).toBeVisible();
  await expect.element(screen.getByRole('button', { name: /On for everyone/ })).toBeVisible();
  await expect.element(screen.getByRole('textbox', { name: 'Notes' })).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Cancel' })).not.toBeInTheDocument();
  await screen.getByRole('button', { name: 'Tasks' }).click();
  await screen.getByRole('button', { name: /Fix it/ }).click();
  await expect.element(screen.getByRole('heading', { name: 'Fix it' })).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Cancel' })).toBeVisible();
});
