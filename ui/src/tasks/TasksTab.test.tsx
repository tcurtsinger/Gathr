import { useState } from 'react';
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { PersonTask } from '../api';
import { type Draft, toAnswer } from './model';
import { type Refusal, TasksTab } from './TasksTab';

const ask = (number: number, what: string, form: PersonTask['form'], labels: string[] = []): PersonTask => ({
  number,
  chat: 'c1',
  chat_title: 'Reconnect',
  giver: 'mira',
  owner: null,
  given_in: 't1',
  kind: 'person',
  what,
  state: 'ready',
  reason: null,
  after: [],
  for_agent: null,
  why: null,
  form,
  choices: labels.map((label) => ({ label })),
  files: [],
  created_us: 0,
});

const tasks = [
  ask(1, 'Keep the partial reply?', 'choice', ['Keep it', 'Start fresh']),
  ask(2, 'Which cases fail?', 'choices', ['Partial', 'Repeated', 'Expired']),
  ask(3, 'What should the notice say?', 'text'),
];

/** The tab as the app holds it: drafts live outside it, and Send settles what the service takes. */
function Tab({ refuse = [] }: { refuse?: Refusal[] }) {
  const [open, setOpen] = useState(tasks);
  const [selected, setSelected] = useState<number | null>(null);
  const [drafts, setDrafts] = useState<Record<number, Draft>>({});
  const [sent, setSent] = useState('');
  return (
    <>
      <TasksTab
        tasks={open}
        selected={selected}
        onSelect={setSelected}
        drafts={drafts}
        onDraft={(n, d) => setDrafts((all) => ({ ...all, [n]: d }))}
        name={() => 'Mira'}
        onOpenFile={() => {}}
        onReference={() => {}}
        onSend={async (ready) => {
          setSent(JSON.stringify(ready.map((t) => toAnswer(t, drafts[t.number]))));
          const taken = ready.filter((t) => !refuse.some((r) => r.task === t.number));
          setOpen((list) => list.filter((t) => !taken.includes(t)));
          return refuse;
        }}
      />
      <output aria-label="Sent">{sent}</output>
    </>
  );
}

test('Back and Next keep drafts, and Send answers only the ready questions', async () => {
  const screen = await render(<Tab />);
  await screen.getByRole('button', { name: /Start fresh/ }).click();
  await screen.getByRole('button', { name: 'Next' }).click();
  // Both questions have Notes: wait for the second before typing.
  await expect.element(screen.getByRole('heading', { name: 'Which cases fail?' })).toBeVisible();
  // Notes alone don't answer a pick: this one stays open.
  await screen.getByRole('textbox', { name: 'Notes (optional)' }).fill('not sure yet');
  await screen.getByRole('button', { name: 'Back' }).click();
  await expect
    .element(screen.getByRole('button', { name: /Start fresh/ }))
    .toHaveAttribute('aria-pressed', 'true');
  await screen.getByRole('button', { name: 'Next' }).click();
  await expect.element(screen.getByRole('textbox', { name: 'Notes (optional)' })).toHaveValue('not sure yet');
  await screen.getByRole('button', { name: 'Next' }).click();
  await screen.getByRole('textbox', { name: /Your answer/ }).fill('Reconnecting');
  await expect.element(screen.getByText('2 ready')).toBeVisible();
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect
    .element(screen.getByLabelText('Sent'))
    .toHaveTextContent('[{"task":1,"pick":["Start fresh"]},{"task":3,"text":"Reconnecting"}]');
  await expect.element(screen.getByRole('heading', { name: 'Which cases fail?' })).toBeVisible();
  await expect.element(screen.getByRole('textbox', { name: 'Notes (optional)' })).toHaveValue('not sure yet');
});

// Focus that lands a frame later can take it from the field being typed in: WebKit lost the answer
// that way (CI runs 42741029642, 42749442823, 42882378961). Frames are held to keep that frame late.
test('Back and Next focus the new heading before the next frame', async () => {
  const held: FrameRequestCallback[] = [];
  const frame = window.requestAnimationFrame;
  window.requestAnimationFrame = (callback) => held.push(callback);
  try {
    const screen = await render(<Tab />);
    await screen.getByRole('button', { name: 'Next' }).click();
    await expect.element(screen.getByRole('heading', { name: 'Which cases fail?' })).toHaveFocus();
    await screen.getByRole('button', { name: 'Back' }).click();
    await expect.element(screen.getByRole('heading', { name: 'Keep the partial reply?' })).toHaveFocus();
  } finally {
    window.requestAnimationFrame = frame;
    for (const callback of held) callback(performance.now());
  }
});

test('Send shows the first refused question, still open, with why', async () => {
  const screen = await render(<Tab refuse={[{ task: 2, why: '#2 was cancelled', ref: 'r-1' }]} />);
  await screen.getByRole('button', { name: 'Next' }).click();
  await screen.getByRole('checkbox', { name: /Partial/ }).click();
  await screen.getByRole('button', { name: 'Next' }).click();
  await screen.getByRole('textbox', { name: /Your answer/ }).fill('Reconnecting');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect.element(screen.getByRole('heading', { name: 'Which cases fail?' })).toBeVisible();
  await expect.element(screen.getByText('Answer failed: #2 was cancelled')).toBeVisible();
});
