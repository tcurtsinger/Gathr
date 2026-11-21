// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Corner } from '../components/Corner';
import { AutoCompact } from './AutoCompact';

afterEach(() => vi.unstubAllGlobals());

test("auto-compact saves when picked, Engine's own as none; a refused pick says why and goes back", async () => {
  const sent: unknown[] = [];
  let refuse = false;
  let point: number | null = 500000;
  vi.stubGlobal('fetch', async (_path: string, init?: RequestInit) => {
    const shown = () => new Response(JSON.stringify({ stale_days: 3, cancel_days: 7, auto_compact: point }));
    if (init?.method !== 'PATCH') return shown();
    const body = JSON.parse(String(init.body));
    sent.push(body);
    if (refuse) {
      const error = { error: 'Settings failed', why: 'made-up refusal', ref: 'made-up-ref' };
      return new Response(JSON.stringify(error), { status: 400 });
    }
    point = body.auto_compact;
    return shown();
  });
  const opened: string[] = [];
  const screen = await render(
    <>
      <AutoCompact />
      <Corner onReference={(ref) => opened.push(ref)} />
    </>,
  );
  const pick = screen.getByRole('combobox', { name: 'Auto-compact' });
  const choose = async (choice: string) => {
    await pick.click();
    await page.getByRole('option', { name: choice, exact: true }).click();
  };
  await expect.element(pick).toHaveTextContent('500K (default)');

  await choose('200K');
  await expect.poll(() => sent).toEqual([{ auto_compact: 200000 }]);
  await choose("Engine's own");
  await expect.poll(() => sent).toEqual([{ auto_compact: 200000 }, { auto_compact: null }]);
  await expect.element(pick).toHaveTextContent("Engine's own");

  refuse = true;
  await choose('300K');
  await expect.element(screen.getByText('made-up refusal')).toBeVisible();
  await screen.getByRole('button', { name: 'Reference made-up-ref' }).click();
  expect(opened).toEqual(['made-up-ref']);
  await expect.element(pick).toHaveTextContent("Engine's own");
});
