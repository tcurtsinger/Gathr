// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { General } from './General';

/** Answers GET /api/startup with `on`; each PUT takes the next answer in `puts`. */
function service(on: boolean, puts: (boolean | 'fail')[], sent: unknown[]) {
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/settings') return new Response(JSON.stringify({ stale_days: 3, cancel_days: 7 }));
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

test('a read on focus that answers after a save never undoes the save', async () => {
  let release = () => {};
  let held = false;
  vi.stubGlobal('fetch', async (_path: string, init?: RequestInit) => {
    if (init?.method === 'PUT') return new Response(JSON.stringify({ on: true }));
    if (!held) return new Response(JSON.stringify({ on: false }));
    // The system's answer is taken now, before the save; the reply waits until released.
    const answer = new Response(JSON.stringify({ on: false }));
    await new Promise<void>((done) => {
      release = done;
    });
    return answer;
  });
  const screen = await render(<General onReference={() => {}} />);
  const toggle = screen.getByRole('switch', { name: 'Open at login' });
  await expect.element(toggle).toHaveAttribute('aria-checked', 'false');

  held = true;
  window.dispatchEvent(new Event('focus'));
  await toggle.click();
  await expect.element(toggle).not.toHaveAttribute('aria-disabled', 'true');
  release();
  await new Promise((done) => setTimeout(done, 50));
  await expect.element(toggle).toHaveAttribute('aria-checked', 'true');
});

test('a read on focus that fails while a save is pending leaves no failure once the save succeeds', async () => {
  let release = () => {};
  let holdPut = false;
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/settings') return new Response(JSON.stringify({ stale_days: 3, cancel_days: 7 }));
    if (init?.method !== 'PUT') {
      if (!holdPut) return new Response(JSON.stringify({ on: false }));
      const body = { error: 'Open at login failed', why: 'made-up read failure', ref: 'made-up-read-ref' };
      return new Response(JSON.stringify(body), { status: 500 });
    }
    await new Promise<void>((done) => {
      release = done;
    });
    return new Response(JSON.stringify({ on: true }));
  });
  const screen = await render(<General onReference={() => {}} />);
  const toggle = screen.getByRole('switch', { name: 'Open at login' });
  await expect.element(toggle).toHaveAttribute('aria-checked', 'false');

  holdPut = true;
  await toggle.click();
  window.dispatchEvent(new Event('focus'));
  await new Promise((done) => setTimeout(done, 50));
  release();
  await expect.element(toggle).not.toHaveAttribute('aria-disabled', 'true');
  await new Promise((done) => setTimeout(done, 50));
  await expect.element(toggle).toHaveAttribute('aria-checked', 'true');
  expect(screen.container.textContent).not.toContain('Open at login failed');
});
