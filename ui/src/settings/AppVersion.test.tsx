// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { AppState } from '../api';
import { AppVersion } from './AppVersion';

vi.mock('../live', () => ({ listen: () => () => {} }));
afterEach(() => vi.unstubAllGlobals());

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });
const sent: unknown[] = [];
/** Answers `GET /api/app` with `app`, and records what each `POST /api/app/update` carried. */
function service(app: Partial<AppState>) {
  sent.length = 0;
  const answer = { version: '9.9.8', state: 'idle', update: null, failed: null, ...app };
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/app/update') sent.push(init?.body ? JSON.parse(String(init.body)) : null);
    return json(path === '/api/app' ? answer : {});
  });
}

test('the version shows, with Update when a release is ready', async () => {
  service({ state: 'ready', update: { version: '9.9.9' } });
  const screen = await render(<AppVersion onReference={() => {}} />);
  await expect.element(screen.getByRole('heading', { name: 'Version 9.9.8' })).toBeVisible();
  await expect.element(screen.getByText('9.9.9 is ready')).toBeVisible();
  await screen.getByRole('button', { name: 'Update to 9.9.9' }).click();
  await expect.poll(() => sent).toEqual([{ install: true }]);
});

test('with nothing ready it says so and checks when asked', async () => {
  service({});
  const screen = await render(<AppVersion onReference={() => {}} />);
  await expect.element(screen.getByText('Up to date')).toBeVisible();
  await screen.getByRole('button', { name: 'Check for updates' }).click();
  await expect.poll(() => sent).toEqual([null]);
});

test('the last failure shows with its reference, and a build with no feed offers nothing', async () => {
  service({ failed: { why: 'made-up failure', ref: 'made-up-ref' } });
  const opened: string[] = [];
  const screen = await render(<AppVersion onReference={(ref) => opened.push(ref)} />);
  await expect.element(screen.getByText('Update failed')).toBeVisible();
  await screen.getByRole('button', { name: 'Reference made-up-ref' }).click();
  expect(opened).toEqual(['made-up-ref']);
  screen.unmount();

  service({ state: 'off' });
  const off = await render(<AppVersion onReference={() => {}} />);
  await expect.element(off.getByText('This build does not update')).toBeVisible();
  await expect.element(off.getByRole('button')).not.toBeInTheDocument();
});
