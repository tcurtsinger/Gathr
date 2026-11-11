// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import { HandoffLimit } from './HandoffLimit';

afterEach(() => vi.unstubAllGlobals());

test('the hand-off limit saves when picked, No limit as none; a refused pick says why and goes back', async () => {
  const sent: unknown[] = [];
  let refuse = false;
  let limit: number | null = 50;
  vi.stubGlobal('fetch', async (_path: string, init?: RequestInit) => {
    const shown = () => new Response(JSON.stringify({ stale_days: 3, cancel_days: 7, handoff_limit: limit }));
    if (init?.method !== 'PATCH') return shown();
    const body = JSON.parse(String(init.body));
    sent.push(body);
    if (refuse) {
      const error = { error: 'Settings failed', why: 'made-up refusal', ref: 'made-up-ref' };
      return new Response(JSON.stringify(error), { status: 400 });
    }
    limit = body.handoff_limit;
    return shown();
  });
  const opened: string[] = [];
  const screen = await render(<HandoffLimit onReference={(ref) => opened.push(ref)} />);
  const pick = screen.getByRole('combobox', { name: 'Pause after' });
  await expect.element(pick).toHaveValue('50');

  await pick.selectOptions('10');
  await expect.poll(() => sent).toEqual([{ handoff_limit: 10 }]);
  await pick.selectOptions('none');
  await expect.poll(() => sent).toEqual([{ handoff_limit: 10 }, { handoff_limit: null }]);
  await expect.element(pick).toHaveValue('none');

  refuse = true;
  await pick.selectOptions('5');
  await expect.element(screen.getByText('made-up refusal')).toBeVisible();
  await screen.getByRole('button', { name: 'Reference made-up-ref' }).click();
  expect(opened).toEqual(['made-up-ref']);
  await expect.element(pick).toHaveValue('none');
});
