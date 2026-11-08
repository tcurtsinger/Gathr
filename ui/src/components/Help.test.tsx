import { useEffect, useRef, useState } from 'react';
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Help } from './Help';

/** Help inside a modal dialog, as Settings holds it: the dialog closes on Escape too. */
function InDialog() {
  const dialog = useRef<HTMLDialogElement>(null);
  const [closed, setClosed] = useState(false);
  useEffect(() => dialog.current?.showModal(), []);
  return (
    <>
      <dialog ref={dialog} onCancel={() => setClosed(true)}>
        <Help label="About made-up">Made-up detail</Help>
      </dialog>
      <p>{closed ? 'Dialog closed' : 'Dialog open'}</p>
    </>
  );
}

// From the keyboard: Safari never focuses a clicked button, so only there is focus the ?'s to keep.
test('Enter on the ? opens its detail; Escape closes the detail, not the dialog, and focus stays on the ?', async () => {
  const screen = await render(<InDialog />);
  const open = screen.getByRole('button', { name: 'About made-up' });
  const detail = screen.getByRole('region', { name: 'About made-up' });
  await expect.element(detail).not.toBeInTheDocument();

  open.element().focus();
  await userEvent.keyboard('{Enter}');
  await expect.element(detail).toBeVisible();
  await expect.element(open).toHaveAttribute('aria-expanded', 'true');

  await userEvent.keyboard('{Escape}');
  await expect.element(detail).not.toBeInTheDocument();
  await expect.element(open).toHaveAttribute('aria-expanded', 'false');
  await expect.element(open).toHaveFocus();
  await expect.element(screen.getByText('Dialog open')).toBeVisible();

  await userEvent.keyboard('{Escape}');
  await expect.element(screen.getByText('Dialog closed')).toBeVisible();
});
