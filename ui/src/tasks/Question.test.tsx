// The one question block (0065): answered in place, each question by itself. With no Notes field a pick answers at
// once; with one a pick marks the choice and Send sends it. Made-up team and questions.
import { useState } from 'react';
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { PersonTask } from '../api';
import { type Draft, type Refusal, toAnswer } from './model';
import { Question } from './Question';

const ask = (more: Partial<PersonTask>): PersonTask => ({
  number: 1,
  chat: 'c1',
  chat_title: 'Recipe import',
  giver: 'mira',
  owner: null,
  given_in: 't1',
  kind: 'person',
  what: 'Keep the placeholder, or use a step’s photo?',
  state: 'ready',
  reason: null,
  after: [],
  for_agent: null,
  why: null,
  form: 'choice',
  choices: [{ label: 'Keep the placeholder' }, { label: 'Use a step’s photo' }],
  files: [],
  created_us: 0,
  ...more,
});
const gathrs = ask({
  number: 9,
  giver: null,
  what: 'Let it go on?',
  choices: [
    { label: 'Let it go on', detail: 'Its review starts.' },
    { label: 'Send it back', detail: '#14 is blocked, and Mira is told.' },
  ],
});

/** The block as the app holds it: the draft lives outside it, and Send answers what the service takes. */
function Held({
  task,
  refuse,
  sentLabel = 'Sent',
}: {
  task: PersonTask;
  refuse?: Refusal;
  sentLabel?: string;
}) {
  const [draft, setDraft] = useState<Draft>({ values: [], notes: '' });
  const [sent, setSent] = useState<string[]>([]);
  const [opened, setOpened] = useState('');
  return (
    <>
      <Question
        task={task}
        draft={draft}
        onDraft={setDraft}
        name={(a) => (a === 'mira' ? 'Mira' : 'An agent')}
        onSend={async (d) => {
          setSent((all) => [...all, JSON.stringify(toAnswer(task, d))]);
          return refuse ?? null;
        }}
        onReference={setOpened}
      />
      <output aria-label={sentLabel}>{sent.join(' ')}</output>
      <output aria-label="Opened">{opened}</output>
    </>
  );
}

test('with no Notes field a pick answers at once; with one a pick only marks the choice and Send sends it', async () => {
  const screen = await render(
    <>
      <Held task={gathrs} sentLabel="Sent for Gathr" />
      <Held task={ask({})} />
    </>,
  );
  // One field on the page: the agent's question's.
  await expect.element(screen.getByRole('textbox')).toHaveAccessibleName('Notes');
  await screen.getByRole('button', { name: /Send it back/ }).click();
  await expect
    .element(screen.getByLabelText('Sent for Gathr'))
    .toHaveTextContent('{"task":9,"pick":["Send it back"]}');
  await expect.element(screen.getByRole('button', { name: /Let it go on/ })).not.toHaveAccessibleName(/pick/);

  const pick = screen.getByRole('button', { name: /Use a step’s photo/ });
  await pick.click();
  await expect.element(pick).toHaveAttribute('aria-pressed', 'true');
  await expect.element(screen.getByLabelText('Sent', { exact: true })).toHaveTextContent('');
  await screen.getByRole('textbox', { name: 'Notes' }).fill('Only for the finished dish.');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect
    .element(screen.getByLabelText('Sent', { exact: true }))
    .toHaveTextContent('{"task":1,"pick":["Use a step’s photo"],"text":"Only for the finished dish."}');
});

test("notes alone answer an agent's question in words, and its first choice is named as the asker's pick", async () => {
  const screen = await render(
    <Held task={ask({ choices: [{ label: 'Keep it', detail: 'Same look.' }, { label: 'Drop it' }] })} />,
  );
  await expect
    .element(screen.getByRole('button', { name: /Keep it/ }))
    .toHaveAccessibleName(/Mira’s pick · Same look\./);
  await expect.element(screen.getByRole('button', { name: /Drop it/ })).not.toHaveAccessibleName(/pick/);
  // "None of these" is an answer (G152).
  await screen.getByRole('textbox', { name: 'Notes' }).fill('Neither, ask Bo');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect
    .element(screen.getByLabelText('Sent'))
    .toHaveTextContent('{"task":1,"text":"Neither, ask Bo"}');
});

test('Discuss keeps the notes and the message apart, each kept when switched', async () => {
  const screen = await render(<Held task={ask({})} />);
  await screen.getByRole('button', { name: /Use a step’s photo/ }).click();
  await screen.getByRole('textbox', { name: 'Notes' }).fill('Only for the finished dish.');
  await screen.getByRole('switch', { name: 'Discuss' }).click();
  const message = screen.getByRole('textbox', { name: 'Message to Mira' });
  await expect.element(message).toHaveValue('');
  await expect.element(screen.getByRole('button', { name: /Use a step’s photo/ })).not.toBeInTheDocument();
  await message.fill('Which step would it use?');
  await screen.getByRole('switch', { name: 'Discuss' }).click();
  await expect
    .element(screen.getByRole('textbox', { name: 'Notes' }))
    .toHaveValue('Only for the finished dish.');
  await expect
    .element(screen.getByRole('button', { name: /Use a step’s photo/ }))
    .toHaveAttribute('aria-pressed', 'true');
  await screen.getByRole('switch', { name: 'Discuss' }).click();
  await expect
    .element(screen.getByRole('textbox', { name: 'Message to Mira' }))
    .toHaveValue('Which step would it use?');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect
    .element(screen.getByLabelText('Sent'))
    .toHaveTextContent('{"task":1,"discuss":true,"text":"Which step would it use?"}');
});

test('a refused Send says Not sent, why and its reference in place, and keeps what was typed', async () => {
  const refuse = { task: 1, why: 'Mira isn’t in this chat any more.', ref: 'R-4HD2' };
  const screen = await render(<Held task={ask({ form: 'text', choices: [] })} refuse={refuse} />);
  await screen.getByRole('textbox', { name: 'Answer' }).fill('Import');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect.element(screen.getByText('Not sent')).toBeVisible();
  await expect.element(screen.getByText('Mira isn’t in this chat any more.')).toBeVisible();
  await expect.element(screen.getByRole('textbox', { name: 'Answer' })).toHaveValue('Import');
  await screen.getByRole('button', { name: 'R-4HD2' }).click();
  await expect.element(screen.getByLabelText('Opened')).toHaveTextContent('R-4HD2');
});
