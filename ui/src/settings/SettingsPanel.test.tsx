import { useState } from 'react';
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
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
        onClose={() => setTarget(null)}
      />
    </>
  );
}

test('Close and Escape both close Settings and give focus back to what opened it', async () => {
  const screen = await render(<Screen />);
  const opener = screen.getByRole('button', { name: 'Settings' });

  await opener.click();
  await screen.getByRole('button', { name: 'Diagnostics' }).click();
  await screen.getByRole('button', { name: 'Close' }).click();
  await expect.element(screen.getByRole('dialog')).not.toBeInTheDocument();
  await expect.element(opener).toHaveFocus();

  await opener.click();
  await expect.element(screen.getByRole('dialog')).toBeVisible();
  await userEvent.keyboard('{Escape}');
  await expect.element(screen.getByRole('dialog')).not.toBeInTheDocument();
  await expect.element(opener).toHaveFocus();
});
