import { useState } from 'react';
import { expect, test } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import '../theme.css';
import { SettingsPanel, type SettingsTarget } from './SettingsPanel';

function Screen() {
  const [target, setTarget] = useState<SettingsTarget | null>(null);
  return (
    <>
      <button type="button" onClick={(e) => setTarget({ page: 'engines', opener: e.currentTarget })}>
        Settings
      </button>
      <SettingsPanel
        target={target}
        onPage={(page) => setTarget((t) => t && { ...t, page })}
        onReference={() => {}}
        onClose={() => setTarget(null)}
      />
    </>
  );
}

test('the X and Escape both close Settings and give focus back to what opened it', async () => {
  const screen = await render(<Screen />);
  const opener = screen.getByRole('button', { name: 'Settings' });

  await opener.click();
  await screen.getByRole('button', { name: 'Diagnostics' }).click();
  await screen.getByRole('button', { name: 'Close Settings' }).click();
  await expect.element(screen.getByRole('dialog')).not.toBeInTheDocument();
  await expect.element(opener).toHaveFocus();

  await opener.click();
  await expect.element(screen.getByRole('dialog')).toBeVisible();
  await userEvent.keyboard('{Escape}');
  await expect.element(screen.getByRole('dialog')).not.toBeInTheDocument();
  await expect.element(opener).toHaveFocus();
});

test('the X names itself in a tooltip inside the dialog, at 1024 too; Escape closes the tooltip, then Settings', async () => {
  await page.viewport(1024, 900);
  const screen = await render(<Screen />);
  const opener = screen.getByRole('button', { name: 'Settings' });
  await opener.click();
  await screen.getByRole('button', { name: 'Close Settings' }).hover();

  const dialog = document.querySelector('dialog') as HTMLDialogElement;
  const tip = page.elementLocator(dialog).getByText('Close Settings', { exact: true }).first();
  await expect.element(tip).toBeVisible();
  // The modal's top layer covers anything outside it, so the tooltip must be inside the dialog.
  const shown = document.querySelector('[data-slot="tooltip-content"]') as HTMLElement;
  expect(dialog.contains(shown)).toBe(true);
  const [panel, box] = [dialog.getBoundingClientRect(), shown.getBoundingClientRect()];
  expect(box.right).toBeLessThanOrEqual(panel.right);
  expect(box.top).toBeGreaterThanOrEqual(panel.top);

  await userEvent.keyboard('{Escape}');
  await expect.poll(() => document.querySelector('[data-slot="tooltip-content"]')).toBeNull();
  expect(dialog.open).toBe(true);
  await userEvent.keyboard('{Escape}');
  await expect.element(screen.getByRole('dialog')).not.toBeInTheDocument();
  await expect.element(opener).toHaveFocus();
});
