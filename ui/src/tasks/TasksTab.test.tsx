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
function Tab({ refuse = [], list = tasks }: { refuse?: Refusal[]; list?: PersonTask[] }) {
  const [open, setOpen] = useState(list);
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
        name={(agent) => (agent ? 'Mira' : 'An agent')}
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

test('Back and Next keep drafts, and notes alone answer a question in words', async () => {
  const screen = await render(<Tab />);
  await screen.getByRole('button', { name: /Start fresh/ }).click();
  await screen.getByRole('button', { name: 'Next' }).click();
  // Both questions have Notes: wait for the second before typing.
  await expect.element(screen.getByRole('heading', { name: 'Which cases fail?' })).toBeVisible();
  // Notes alone answer it in words: "none of these" is an answer (owner, G152).
  await screen.getByRole('textbox', { name: 'Notes (optional)' }).fill('not sure yet');
  await screen.getByRole('button', { name: 'Back' }).click();
  await expect
    .element(screen.getByRole('button', { name: /Start fresh/ }))
    .toHaveAttribute('aria-pressed', 'true');
  await screen.getByRole('button', { name: 'Next' }).click();
  await expect.element(screen.getByRole('textbox', { name: 'Notes (optional)' })).toHaveValue('not sure yet');
  await screen.getByRole('button', { name: 'Next' }).click();
  await screen.getByRole('textbox', { name: /Your answer/ }).fill('Reconnecting');
  await expect.element(screen.getByText('3 ready')).toBeVisible();
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect
    .element(screen.getByLabelText('Sent'))
    .toHaveTextContent(
      '[{"task":1,"pick":["Start fresh"]},{"task":2,"text":"not sure yet"},{"task":3,"text":"Reconnecting"}]',
    );
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

// Gathr's own question (giver null) is answered only by one of its choices, and Gathr has no pick to recommend.
test('gathrs_question_names_gathr_offers_no_recommended_and_needs_a_pick', async () => {
  const question: PersonTask = {
    ...ask(9, "#5 adds 3,200 lines, over this project's line of 3,000: let it go on?", 'choice'),
    giver: null,
    given_in: null,
    pr: 5,
    choices: [
      { label: 'Let it go on', detail: 'It lands once its review and checks pass.' },
      { label: 'Send it back', detail: '#1 is blocked, and Lead is told.' },
    ],
  };
  const screen = await render(<Tab list={[question]} />);
  await expect.element(screen.getByText('Gathr', { exact: true })).toBeVisible();
  const go = screen.getByRole('button', { name: /Let it go on/ });
  await expect.element(go).toHaveAccessibleName(/It lands once its review and checks pass\./);
  await expect.element(go).not.toHaveAccessibleName(/Recommended/);
  await screen.getByRole('textbox', { name: 'Notes (optional)' }).fill('looks fine');
  await expect.element(screen.getByText('0 ready')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
  await go.click();
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect
    .element(screen.getByLabelText('Sent'))
    .toHaveTextContent('[{"task":9,"pick":["Let it go on"],"text":"looks fine"}]');
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
