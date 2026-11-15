// The size line opens to the ask's two sizes; choosing the other one asks the service to change it (0061).
import type { ThreadMessageLike } from '@assistant-ui/react';
import { afterEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Custom, EventLine } from './messages';
import { type Actions, Thread } from './Thread';

afterEach(() => vi.unstubAllGlobals());

const none: Actions = {
  openTask: () => {},
  openAgent: () => {},
  diagnostics: () => {},
  reference: () => {},
  signIn: () => {},
  retry: () => {},
  resend: () => {},
  carryOn: () => {},
  approve: () => {},
};

function sized(line: EventLine) {
  const custom: Custom = { events: [line] };
  const item: ThreadMessageLike = {
    id: `size-${line.ask}`,
    role: 'system',
    content: [{ type: 'text', text: `${line.who} ${line.text}` }],
    metadata: { custom },
  };
  return render(
    <div style={{ height: 600, display: 'flex' }}>
      <Thread messages={[item]} running={false} actions={none} empty={false}>
        <p>Composer</p>
      </Thread>
    </div>,
  );
}

/** The service, stubbed: each POST it gets, and what it answers. */
function service(answer: () => Response) {
  const sent: { path: string; body: unknown }[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    sent.push({ path, body: init?.body ? JSON.parse(String(init.body)) : undefined });
    return answer();
  });
  return sent;
}

const fix: EventLine = { kind: 'fix', who: 'Mira:', text: 'Fix', ask: 'm1' };
const asked = {
  id: 'm1',
  title: 'The export button does nothing',
  size: 'new',
  by: 'person',
  sized_in: null,
};

test('the line opens on click and on Enter to the two sizes, the current one pressed', async () => {
  const screen = await sized(fix);
  const trigger = screen.getByRole('button', { name: 'Mira: Fix' });
  await trigger.click();
  const group = screen.getByRole('group', { name: 'Size' });
  await expect.element(group).toBeVisible();
  await expect
    .element(group.getByRole('button', { name: 'Just fix it.' }))
    .toHaveAttribute('aria-pressed', 'true');
  await expect
    .element(group.getByRole('button', { name: 'Plan it with me first.' }))
    .toHaveAttribute('aria-pressed', 'false');

  await trigger.click();
  await expect.element(group).not.toBeInTheDocument();
  (trigger.element() as HTMLElement).focus();
  await userEvent.keyboard('{Enter}');
  await expect.element(screen.getByRole('group', { name: 'Size' })).toBeVisible();
});

test('choosing the other size asks the service; choosing the pressed one does nothing', async () => {
  const sent = service(() => new Response(JSON.stringify(asked)));
  const screen = await sized(fix);
  await screen.getByRole('button', { name: 'Mira: Fix' }).click();
  await screen.getByRole('button', { name: 'Just fix it.' }).click();
  await screen.getByRole('button', { name: 'Plan it with me first.' }).click();
  await expect.poll(() => sent).toEqual([{ path: '/api/asks/m1/size', body: { size: 'new' } }]);
});

test('a refused size shows why, with its reference', async () => {
  const refusal = { error: 'Size failed', why: 'chat c is archived', ref: '01REFSIZE' };
  service(() => new Response(JSON.stringify(refusal), { status: 409 }));
  const screen = await sized(fix);
  await screen.getByRole('button', { name: 'Mira: Fix' }).click();
  await screen.getByRole('button', { name: 'Plan it with me first.' }).click();
  const alert = screen.getByRole('alert');
  // The title, the reference, then why.
  await expect.element(alert).toHaveTextContent('Size failed01REFSIZEchat c is archived');
  await expect.element(alert.getByRole('button', { name: 'Reference 01REFSIZE' })).toBeVisible();
});
