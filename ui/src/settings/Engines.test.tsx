// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Account, Engine } from '../api';
import { Engines } from './Engines';

const account = (id: string, name: string): Account => ({
  id,
  name,
  engine: 'claude',
  login: 'signed_out',
  expires: null,
  sign_in: null,
});

/** Answers GET /api/accounts and /api/engines/claude; each engine answer is the next in `copies`. */
function service(copies: Engine['copy'][]) {
  const answer = (body: unknown) => new Response(JSON.stringify(body), { status: 200 });
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/accounts')
      return answer({ accounts: [account('a', 'Made-up one'), account('b', 'Made-up two')] });
    if (path === '/api/engines/claude') {
      const copy = copies.length > 1 ? copies.shift() : copies[0];
      return answer({ found: 'claude', why_not_found: null, copy });
    }
    return answer({});
  });
}
afterEach(() => vi.unstubAllGlobals());

const copy = (state: Engine['copy']['state'], failed: Engine['copy']['failed'] = null): Engine['copy'] => ({
  version: '2.1.320',
  state,
  checked: state === 'checking' ? null : '2026-11-08T00:00:00Z',
  failed,
});

test('Rename on another account opens its own name, not the last draft', async () => {
  service([copy('idle')]);
  const screen = await render(<Engines onReference={() => {}} />);
  const rows = screen.getByRole('listitem');
  await rows.filter({ hasText: 'Made-up one' }).getByRole('button', { name: 'Rename' }).click();
  await screen.getByRole('textbox', { name: 'Name' }).fill('Replacement');
  await rows.filter({ hasText: 'Made-up two' }).getByRole('button', { name: 'Rename' }).click();
  await expect.element(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('Made-up two');
});

test('a check that takes several looks ends as failed, with Check back and no Up to date', async () => {
  const failed = { why: 'made-up feed failure', ref: 'made-up-ref' };
  service([copy('checking'), copy('checking'), copy('checking'), copy('idle', failed)]);
  const screen = await render(<Engines onReference={() => {}} />);
  await expect.element(screen.getByText('Update failed')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Check' })).toBeEnabled();
  await expect.element(screen.getByText('Up to date')).not.toBeInTheDocument();
});
