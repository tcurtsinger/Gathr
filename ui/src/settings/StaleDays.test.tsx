// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { StaleDays } from './StaleDays';

afterEach(() => vi.unstubAllGlobals());

test('a day count saves when the edit is committed; an invalid or refused one stays as typed', async () => {
  const sent: unknown[] = [];
  let refuse = false;
  vi.stubGlobal('fetch', async (_path: string, init?: RequestInit) => {
    if (init?.method !== 'PATCH') return new Response(JSON.stringify({ stale_days: 3, cancel_days: 7 }));
    const body = JSON.parse(String(init.body));
    sent.push(body);
    if (refuse) {
      const error = { error: 'Save failed', why: 'made-up refusal', ref: 'made-up-ref' };
      return new Response(JSON.stringify(error), { status: 500 });
    }
    return new Response(JSON.stringify({ stale_days: 3, cancel_days: 7, ...body }));
  });
  const screen = await render(<StaleDays onReference={() => {}} />);
  const cancel = screen.getByRole('spinbutton', { name: 'Cancel after' });
  await expect.element(cancel).toHaveValue(7);
  await expect.element(cancel).toHaveAccessibleDescription('days stale');

  await userEvent.fill(cancel, '0');
  await userEvent.keyboard('{Enter}');
  await expect.element(screen.getByText('Enter whole days, at least 1.')).toBeVisible();
  await expect.element(cancel).toHaveValue(0);
  expect(sent).toEqual([]);

  await userEvent.fill(cancel, '10');
  await userEvent.tab();
  await expect.element(screen.getByText('Enter whole days, at least 1.')).not.toBeInTheDocument();
  await expect.poll(() => sent).toEqual([{ cancel_days: 10 }]);
  await expect.element(cancel).toHaveValue(10);

  refuse = true;
  await userEvent.fill(cancel, '12');
  await userEvent.keyboard('{Enter}');
  await expect.element(screen.getByText('Save failed')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Reference made-up-ref' })).toBeVisible();
  await expect.element(cancel).toHaveValue(12);
});
