// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Account, Engine } from '../api';
import { Engines } from './Engines';

const account = (id: string, name: string, sign_in: Account['sign_in'] = null): Account => ({
  id,
  name,
  engine: 'claude',
  login: 'signed_out',
  expires: null,
  sign_in,
});

/**
 * Answers GET /api/accounts and each engine: Claude Code's answer is the next in `copies`, Codex's
 * is always up to date. Every request except a GET lands in `sent`, with its body if it has one.
 */
function service(copies: Engine['copy'][], sent: string[] = [], accounts = defaults) {
  const answer = (body: unknown) => new Response(JSON.stringify(body), { status: 200 });
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method && init.method !== 'GET')
      sent.push([init.method, path, init.body].filter(Boolean).join(' '));
    if (path === '/api/accounts') return answer({ accounts });
    if (path === '/api/engines/claude') {
      const copy = copies.length > 1 ? copies.shift() : copies[0];
      return answer({ found: 'claude', why_not_found: null, copy });
    }
    if (path === '/api/engines/codex') return answer({ found: 'codex', why_not_found: null, copy: codex });
    return answer({});
  });
}
const defaults = [account('a', 'Made-up one'), account('b', 'Made-up two')];
afterEach(() => vi.unstubAllGlobals());

const copy = (state: Engine['copy']['state'], failed: Engine['copy']['failed'] = null): Engine['copy'] => ({
  version: '2.1.320',
  state,
  checked: state === 'checking' ? null : '2026-11-08T00:00:00Z',
  failed,
});
const codex: Engine['copy'] = { ...copy('idle'), version: '0.166.0' };

test('Rename on another account opens its own name, not the last draft', async () => {
  service([copy('idle')]);
  const screen = await render(<Engines onReference={() => {}} />);
  const rows = screen.getByRole('listitem');
  await rows.filter({ hasText: 'Made-up one' }).getByRole('button', { name: 'Rename' }).click();
  await screen.getByRole('textbox', { name: 'Name' }).fill('Replacement');
  await rows.filter({ hasText: 'Made-up two' }).getByRole('button', { name: 'Rename' }).click();
  await expect.element(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('Made-up two');
});

test('Remove asks first: Escape keeps the account and Settings open, Remove deletes only that one', async () => {
  const removed: string[] = [];
  service([copy('idle')], removed);
  const screen = await render(
    <dialog ref={(d) => d?.showModal()}>
      <Engines onReference={() => {}} />
    </dialog>,
  );
  const row = screen.getByRole('listitem').filter({ hasText: 'Made-up two' });
  const remove = row.getByRole('button', { name: 'Remove' });

  await remove.click();
  const question = row.getByRole('group', { name: 'Remove Made-up two?' });
  await expect.element(question.getByRole('button', { name: 'Cancel' })).toHaveFocus();
  await userEvent.keyboard('{Escape}');
  await expect.element(question).not.toBeInTheDocument();
  await expect.element(remove).toHaveFocus();
  expect(screen.container.querySelector('dialog')?.open).toBe(true);
  expect(removed).toEqual([]);

  await remove.click();
  await question.getByRole('button', { name: 'Remove' }).click();
  await expect.poll(() => removed).toEqual(['DELETE /api/accounts/b']);
  await expect.element(screen.getByRole('button', { name: 'Add' })).toHaveFocus();
});

test('a check that takes several looks ends as failed, with Check back and no Up to date', async () => {
  const failed = { why: 'made-up feed failure', ref: 'made-up-ref' };
  service([copy('checking'), copy('checking'), copy('checking'), copy('idle', failed)]);
  const screen = await render(<Engines onReference={() => {}} />);
  await expect.element(screen.getByText('Update failed')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Check' }).first()).toBeEnabled();
  // Only Codex, listed second, is up to date.
  await expect.element(screen.getByText('Up to date')).toBeVisible();
  expect(screen.getByText('Up to date').elements()).toHaveLength(1);
});

test("each engine's Check asks for its own copy", async () => {
  const sent: string[] = [];
  service([copy('idle')], sent);
  const screen = await render(<Engines onReference={() => {}} />);
  // Codex is listed second.
  await screen.getByRole('button', { name: 'Check' }).nth(1).click();
  await expect.poll(() => sent).toEqual(['POST /api/engines/codex/update']);
});

test('a sign-in shows the code field only when its engine takes a pasted code', async () => {
  const page = 'https://example.invalid/sign-in';
  service(
    [copy('idle')],
    [],
    [
      account('a', 'Made-up paste', { state: 'waiting', url: page, paste: true }),
      { ...account('b', 'Made-up browser', { state: 'waiting', url: page, paste: false }), engine: 'codex' },
    ],
  );
  const opened: unknown[] = [];
  vi.stubGlobal('open', (url: unknown) => opened.push(url));
  const screen = await render(<Engines onReference={() => {}} />);
  const rows = screen.getByRole('listitem');
  const paste = rows.filter({ hasText: 'Made-up paste' });
  const browser = rows.filter({ hasText: 'Made-up browser' });
  await expect.element(paste.getByRole('textbox', { name: 'Code' })).toBeVisible();
  await expect.element(browser.getByText('Codex / Signing in')).toBeVisible();
  await expect.element(browser.getByRole('textbox', { name: 'Code' })).not.toBeInTheDocument();
  await browser.getByRole('button', { name: 'Open sign-in page' }).click();
  expect(opened).toEqual([page]);
});

test('a Codex sign-in moves between the browser page and a device code, and a refused code offers Retry', async () => {
  const page = 'https://example.invalid/sign-in';
  const codex = (id: string, name: string, sign_in: Account['sign_in']) => ({
    ...account(id, name, sign_in),
    engine: 'codex',
  });
  const sent: string[] = [];
  service([copy('idle')], sent, [
    codex('a', 'Made-up page', { state: 'waiting', url: page }),
    codex('b', 'Made-up device', { state: 'waiting', url: page, user_code: 'MADE-UP1' }),
    codex('c', 'Made-up refused', {
      state: 'failed',
      why: 'made-up refusal',
      ref: 'made-up-ref',
      device: true,
    }),
  ]);
  const screen = await render(<Engines onReference={() => {}} />);
  const rows = screen.getByRole('listitem');

  await rows.filter({ hasText: 'Made-up page' }).getByRole('button', { name: 'Use a device code' }).click();
  const device = rows.filter({ hasText: 'Made-up device' });
  await expect.element(device.getByText('MADE-UP1')).toBeVisible();
  await expect.element(device.getByRole('button', { name: 'Use a device code' })).not.toBeInTheDocument();
  await device.getByRole('button', { name: 'Use browser' }).click();
  const refused = rows.filter({ hasText: 'Made-up refused' });
  await expect.element(refused.getByText('made-up refusal')).toBeVisible();
  await refused.getByRole('button', { name: 'Retry' }).click();
  await expect
    .poll(() => sent)
    .toEqual([
      'POST /api/accounts/a/sign-in {"device":true}',
      'POST /api/accounts/b/sign-in',
      'POST /api/accounts/c/sign-in {"device":true}',
    ]);
});
