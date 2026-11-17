import { useState } from 'react';
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Confirm } from './Confirm';
import { Dialog } from './Dialog';

const cancelled: string[] = [];

/** Settings, as a dialog, with a Delete that opens the confirm over it. */
function Settings() {
  const [asking, setAsking] = useState(false);
  return (
    <Dialog open title="Made-up settings" onClose={() => cancelled.push('settings')}>
      <button type="button" onClick={() => setAsking(true)}>
        Delete
      </button>
      {asking && (
        <Confirm
          question="Delete Made-up?"
          body="2 made-up things will be deleted."
          verb="Delete"
          danger
          onCancel={() => {
            cancelled.push('confirm');
            setAsking(false);
          }}
          onConfirm={async () => {}}
          onReference={() => {}}
        />
      )}
    </Dialog>
  );
}

test('a confirm shows its body with Cancel focused, and Escape cancels it alone, not the dialog under it', async () => {
  const screen = await render(<Settings />);
  await screen.getByRole('button', { name: 'Delete' }).click();
  const confirm = screen.getByRole('dialog', { name: 'Delete Made-up?' });
  await expect.element(confirm.getByText('2 made-up things will be deleted.')).toBeVisible();
  await expect.element(confirm.getByRole('button', { name: 'Cancel' })).toHaveFocus();
  await userEvent.keyboard('{Escape}');
  await expect.element(confirm).not.toBeInTheDocument();
  expect(cancelled).toEqual(['confirm']);
});
