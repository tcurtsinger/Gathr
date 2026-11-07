import { useState } from 'react';
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Composer } from './Composer';

/** The composer as a chat holds it: sending clears the draft and lists what was sent. */
function Chat({ running = false }: { running?: boolean }) {
  const [draft, setDraft] = useState('');
  const [sent, setSent] = useState<string[]>([]);
  const [stopped, setStopped] = useState(false);
  return (
    <>
      <Composer
        draft={draft}
        onDraft={setDraft}
        onSend={() => {
          setSent([...sent, draft.replaceAll('\n', '/')]);
          setDraft('');
        }}
        onStop={running ? () => setStopped(true) : undefined}
      />
      <output aria-label="Sent">{sent.join('|')}</output>
      {stopped && <p>Stopped</p>}
    </>
  );
}

test('Enter sends the draft and Shift+Enter starts a new line', async () => {
  const screen = await render(<Chat />);
  await screen.getByRole('textbox', { name: 'Message' }).click();
  await userEvent.keyboard('first{Shift>}{Enter}{/Shift}second{Enter}');
  await expect.element(screen.getByText('first/second', { exact: true })).toBeVisible();
  await expect.element(screen.getByRole('textbox', { name: 'Message' })).toHaveValue('');
});

test('while a turn runs the button stops it, and typing turns it back into Send', async () => {
  const screen = await render(<Chat running />);
  await screen.getByRole('button', { name: 'Stop' }).click();
  await expect.element(screen.getByText('Stopped')).toBeVisible();

  await screen.getByRole('textbox', { name: 'Message' }).fill('also check the tests');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect
    .element(screen.getByRole('status', { name: 'Sent' }))
    .toHaveTextContent('also check the tests');
});
