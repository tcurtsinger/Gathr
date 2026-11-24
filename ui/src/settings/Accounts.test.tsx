// No service answers here, so every request fails, as when the service is down.
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Corner } from '../components/Corner';
import { Accounts } from './Accounts';

test('Add opens a row at the name, needs one, keeps it when adding fails, and Cancel closes it', async () => {
  let closed = 0;
  const screen = await render(
    <>
      <Accounts adding onAdded={() => closed++} />
      <Corner onReference={() => {}} />
    </>,
  );
  const name = screen.getByRole('textbox', { name: 'Name' });
  await expect.element(name).toHaveFocus();
  const form = screen.getByRole('form', { name: 'Add account' });
  const submit = form.getByRole('button', { name: 'Add' });
  await expect.element(submit).toBeDisabled();
  await userEvent.keyboard('   ');
  await expect.element(submit).toBeDisabled();

  await name.fill('Made-up work');
  await userEvent.keyboard('{Enter}');
  await expect.element(screen.getByRole('alert').filter({ hasText: 'Request failed' }).last()).toBeVisible();
  await expect.element(name).toHaveValue('Made-up work');
  await expect.element(submit).toBeEnabled();

  await form.getByRole('button', { name: 'Cancel' }).click();
  expect(closed).toBe(1);
});
