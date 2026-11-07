// No service answers here, so every request fails, as when the service is down.
import { useState } from 'react';
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import { ChatView } from './ChatView';
import type { SendFailure } from './rows';

/** Two chats and what the app keeps for each while another is open. */
function TwoChats() {
  const [open, setOpen] = useState('a');
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [unsent, setUnsent] = useState<Record<string, SendFailure[]>>({});
  return (
    <>
      <button type="button" onClick={() => setOpen('a')}>
        Chat A
      </button>
      <button type="button" onClick={() => setOpen('b')}>
        Chat B
      </button>
      <ChatView
        key={open}
        chat={{ id: open, title: `Chat ${open}` }}
        draft={drafts[open] ?? ''}
        onDraft={(text) => setDrafts((d) => ({ ...d, [open]: text }))}
        unsent={unsent[open] ?? []}
        onUnsent={(update) => setUnsent((u) => ({ ...u, [open]: update(u[open] ?? []) }))}
        openSettings={() => {}}
        onNamed={() => {}}
      />
    </>
  );
}

test('a message that never reached the service is still there, with Retry, after switching chats', async () => {
  const screen = await render(<TwoChats />);
  await screen.getByRole('textbox', { name: 'Message' }).fill('Keep this fabricated draft');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect.element(screen.getByText('Keep this fabricated draft', { exact: true })).toBeVisible();

  await screen.getByRole('button', { name: 'Chat B' }).click();
  await expect
    .element(screen.getByText('Keep this fabricated draft', { exact: true }))
    .not.toBeInTheDocument();
  await screen.getByRole('button', { name: 'Chat A' }).click();

  const unsent = screen.getByText('Keep this fabricated draft', { exact: true });
  await expect.element(unsent).toBeVisible();
  await expect.element(screen.getByRole('alert').filter({ hasText: 'Request failed' }).last()).toBeVisible();
});
