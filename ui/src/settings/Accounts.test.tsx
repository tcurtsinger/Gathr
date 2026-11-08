// No service answers here, so every request fails, as when the service is down.
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Accounts } from './Accounts';

test('Add opens at the name, needs one, keeps it when adding fails, and Cancel gives focus back', async () => {
  const screen = await render(<Accounts onReference={() => {}} />);
  const add = screen.getByRole('button', { name: 'Add' });

  await add.click();
  const name = screen.getByRole('textbox', { name: 'Name' });
  await expect.element(name).toHaveFocus();
  const signIn = screen.getByRole('button', { name: 'Sign in' });
  await expect.element(signIn).toBeDisabled();
  await userEvent.keyboard('   ');
  await expect.element(signIn).toBeDisabled();

  await name.fill('Made-up work');
  await userEvent.keyboard('{Enter}');
  await expect.element(screen.getByRole('alert').filter({ hasText: 'Request failed' })).toBeVisible();
  await expect.element(name).toHaveValue('Made-up work');
  await expect.element(signIn).toBeEnabled();

  await screen.getByRole('button', { name: 'Cancel' }).click();
  await expect.element(screen.getByRole('form', { name: 'Add account' })).not.toBeInTheDocument();
  await expect.element(add).toHaveFocus();
});
