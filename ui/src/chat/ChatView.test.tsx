// No service answers here, so every request fails, as when the service is down.
import { useState } from 'react';
import { expect, test, vi } from 'vitest';
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
        agents={[]}
        onEditAgent={() => {}}
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

test('the chat reopens on its last turn’s agent, Edit agent keeps it, and the message goes to it', async () => {
  const sent: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.endsWith('/messages')) sent.push(JSON.parse(String(init.body)));
    const turns = [{ id: 't', status: 'done', why: null, ref: 'r', agent: 'b' }];
    return new Response(JSON.stringify({ chat: 'c', messages: [], turns, approvals: [] }));
  });
  const edited: string[] = [];
  const made = (id: string, name: string) => ({
    id,
    name,
    account: 'a',
    model: null,
    effort: null,
    approvals: 'auto' as const,
  });
  const screen = await render(
    <ChatView
      chat={{ id: 'c', title: 'Made-up chat' }}
      agents={[made('a', 'Made-up A'), made('b', 'Made-up B')]}
      onEditAgent={(agent) => edited.push(agent)}
      draft="Made-up question"
      onDraft={() => {}}
      unsent={[]}
      onUnsent={() => {}}
      openSettings={() => {}}
      onNamed={() => {}}
    />,
  );
  const picker = screen.getByRole('combobox', { name: 'Agent' });
  await expect.element(picker).toHaveValue('b');
  await picker.selectOptions('Edit agent');
  expect(edited).toEqual(['b']);
  await expect.element(picker).toHaveValue('b');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect.poll(() => sent).toEqual([{ text: 'Made-up question', agent: 'b' }]);
  vi.unstubAllGlobals();
});

test('Retry sends a failed turn to the agent it was sent to, whoever is picked now', async () => {
  const sent: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.endsWith('/messages')) {
      sent.push(JSON.parse(String(init.body)));
      return new Response(JSON.stringify({ turn: 'u', status: 'running' }));
    }
    const asked = { id: 'm', turn: 't', time: '', role: 'user', kind: 'text', body: 'Made-up ask' };
    const turns = [{ id: 't', status: 'failed', why: 'made-up', ref: 'r', agent: 'a' }];
    return new Response(JSON.stringify({ chat: 'c', messages: [asked], turns, approvals: [] }));
  });
  const made = (id: string, name: string) => ({
    id,
    name,
    account: 'a',
    model: null,
    effort: null,
    approvals: 'auto' as const,
  });
  const screen = await render(
    <ChatView
      chat={{ id: 'c', title: 'Made-up chat' }}
      agents={[made('a', 'Made-up A'), made('b', 'Made-up B')]}
      onEditAgent={() => {}}
      draft=""
      onDraft={() => {}}
      unsent={[]}
      onUnsent={() => {}}
      openSettings={() => {}}
      onNamed={() => {}}
    />,
  );
  await screen.getByRole('combobox', { name: 'Agent' }).selectOptions('Made-up B');
  await screen.getByRole('button', { name: 'Retry' }).click();
  await expect.poll(() => sent).toEqual([{ text: 'Made-up ask', agent: 'a' }]);
  vi.unstubAllGlobals();
});
