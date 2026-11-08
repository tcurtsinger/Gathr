// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { General } from './General';

/** Answers GET /api/startup with `on`; each PUT takes the next answer in `puts`. */
function service(on: boolean, puts: (boolean | 'fail')[], sent: unknown[]) {
  vi.stubGlobal('fetch', async (_path: string, init?: RequestInit) => {
    if (init?.method !== 'PUT') return new Response(JSON.stringify({ on }));
    sent.push(JSON.parse(String(init.body)));
    const next = puts.shift();
    if (next === 'fail') {
      const body = { error: 'Change failed', why: 'made-up refusal', ref: 'made-up-ref' };
      return new Response(JSON.stringify(body), { status: 500 });
    }
    return new Response(JSON.stringify({ on: next }));
  });
}
afterEach(() => vi.unstubAllGlobals());

test('Space turns it on; a change that fails puts it back with its reference and keeps focus', async () => {
  const sent: unknown[] = [];
  service(false, [true, 'fail'], sent);
  const screen = await render(<General onReference={() => {}} />);
  const toggle = screen.getByRole('switch', { name: 'Open at login' });
  await expect.element(toggle).toHaveAttribute('aria-checked', 'false');
  await expect.element(toggle).toHaveAccessibleDescription('This computer');

  toggle.element().focus();
  await userEvent.keyboard(' ');
  await expect.element(toggle).toHaveAttribute('aria-checked', 'true');
  await expect.element(toggle).not.toHaveAttribute('aria-disabled', 'true');

  await userEvent.keyboard('{Enter}');
  await expect.element(screen.getByText('Change failed')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Reference made-up-ref' })).toBeVisible();
  await expect.element(toggle).toHaveAttribute('aria-checked', 'true');
  await expect.element(toggle).toHaveFocus();
  expect(sent).toEqual([{ on: true }, { on: false }]);
});
