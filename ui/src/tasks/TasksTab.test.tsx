import { useState } from 'react';
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Draft, PersonTask } from './model';
import { TasksTab } from './TasksTab';
import '../theme.css';

const ask = (id: string, title: string, answer: PersonTask['answer'], labels: string[] = []): PersonTask => ({
  id,
  chat: 'c1',
  chatTitle: 'Reconnect',
  agent: 'Mira',
  title,
  answer,
  options: labels.map((label) => ({ id: label, label })),
});

const tasks = [
  ask('1', 'Keep the partial reply?', 'one', ['Keep it', 'Start fresh']),
  ask('2', 'Which cases fail?', 'many', ['Partial', 'Repeated', 'Expired']),
  ask('3', 'What should the notice say?', 'text'),
];

/** The tab as the app holds it: drafts live outside it, and Send settles what it was given. */
function Tab() {
  const [open, setOpen] = useState(tasks);
  const [selected, setSelected] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [sent, setSent] = useState('');
  return (
    <>
      <TasksTab
        tasks={open}
        selected={selected}
        onSelect={setSelected}
        drafts={drafts}
        onDraft={(id, d) => setDrafts((all) => ({ ...all, [id]: d }))}
        onOpenChat={() => {}}
        onSend={async (answers) => {
          setSent(answers.map((a) => `${a.task.id}:${a.draft.values.join('+')}/${a.draft.notes}`).join(' '));
          setOpen((list) => list.filter((t) => !answers.some((a) => a.task.id === t.id)));
          return [];
        }}
      />
      <output aria-label="Sent">{sent}</output>
    </>
  );
}

test('Back and Next keep drafts, and Send answers only the ready tasks', async () => {
  const screen = await render(<Tab />);
  await screen.getByRole('button', { name: 'Start fresh' }).click();
  await screen.getByRole('button', { name: 'Next' }).click();
  // Notes alone don't answer a pick: this one stays open.
  await screen.getByRole('textbox', { name: 'Notes (optional)' }).fill('not sure yet');
  await screen.getByRole('button', { name: 'Back' }).click();
  await expect
    .element(screen.getByRole('button', { name: 'Start fresh' }))
    .toHaveAttribute('aria-pressed', 'true');
  await screen.getByRole('button', { name: 'Next' }).click();
  await expect.element(screen.getByRole('textbox', { name: 'Notes (optional)' })).toHaveValue('not sure yet');
  await screen.getByRole('button', { name: 'Next' }).click();
  await screen.getByRole('textbox', { name: /Your answer/ }).fill('Reconnecting');
  await expect.element(screen.getByText('2 Done')).toBeVisible();
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect.element(screen.getByLabelText('Sent')).toHaveTextContent('1:Start fresh/ 3:Reconnecting/');
  await expect.element(screen.getByRole('heading', { name: 'Which cases fail?' })).toBeVisible();
  await expect.element(screen.getByRole('textbox', { name: 'Notes (optional)' })).toHaveValue('not sure yet');
});
