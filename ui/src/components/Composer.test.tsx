import { useState } from 'react';
import { expect, test } from 'vitest';
import { server, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Composer } from './Composer';
import '../theme.css';

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

test('keyboard focus shows no ring, on the message box or its buttons (DESIGN.md)', async () => {
  const screen = await render(<Chat running />);
  const input = screen.getByRole('textbox', { name: 'Message' });
  await input.click();
  // Safari's Tab skips buttons unless the person turns that on; Option+Tab reaches them.
  await userEvent.keyboard(server.browser === 'webkit' ? '{Alt>}{Tab}{/Alt}' : '{Tab}');
  const stop = screen.getByRole('button', { name: 'Stop this chat now' }).element() as HTMLElement;
  expect(document.activeElement).toBe(stop);
  expect(stop.matches(':focus-visible')).toBe(true);
  for (const el of [
    input.element(),
    input.element().closest('[data-slot="composer-bar"]') as HTMLElement,
    stop,
  ]) {
    expect(getComputedStyle(el).outlineStyle).toBe('none');
    // A ring is a box-shadow in a colour that shows; a cleared one is fully transparent.
    const colours = getComputedStyle(el).boxShadow.match(/(rgba?|oklch|oklab|lab)\([^)]*\)/g) ?? [];
    expect(colours.filter((c) => c !== 'rgba(0, 0, 0, 0)')).toEqual([]);
  }
  // It gets the hover highlight instead, over its own background.
  expect(getComputedStyle(stop).backgroundImage).toContain('gradient');
});

test('while a turn runs the button stops it, and typing turns it back into Send', async () => {
  const screen = await render(<Chat running />);
  await screen.getByRole('button', { name: 'Stop this chat now' }).click();
  await expect.element(screen.getByText('Stopped')).toBeVisible();

  await screen.getByRole('textbox', { name: 'Message' }).fill('also check the tests');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect
    .element(screen.getByRole('status', { name: 'Sent' }))
    .toHaveTextContent('also check the tests');
});
