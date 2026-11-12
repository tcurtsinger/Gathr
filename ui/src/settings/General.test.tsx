// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { followTheme } from '../theme';
import { General } from './General';

// Version and updates have their own test (AppVersion.test.tsx): here the service says nothing of them.
vi.mock('../live', () => ({ listen: () => () => {} }));

/** Answers GET /api/startup with `on`; each PUT takes the next answer in `puts`. */
function service(on: boolean, puts: (boolean | 'fail')[], sent: unknown[]) {
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/settings') return new Response(JSON.stringify({ stale_days: 3, cancel_days: 7 }));
    if (path === '/api/app') return new Response('{}');
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

/** Writes the window's Theme cookie as any port of this host could. */
function cookie(value: string) {
  // biome-ignore lint/suspicious/noDocumentCookie: the test plays a window on another port.
  document.cookie = `gathr.theme=${value}; path=/; max-age=${value ? 60 : 0}`;
}

test('a Theme kept while the service ran on another port is used at start', () => {
  // localStorage is kept per origin, so per port; a cookie is kept per host, which every port reads.
  const systemLight = matchMedia('(prefers-color-scheme: light)').matches;
  cookie(systemLight ? 'dark' : 'light');
  followTheme();
  expect(document.documentElement.classList.contains('light')).toBe(!systemLight);
  cookie('');
  followTheme();
});

test('Theme applies at once, is kept for the next start, and System follows the system again', async () => {
  service(false, [], []);
  cookie('');
  const light = () => document.documentElement.classList.contains('light');
  const systemLight = matchMedia('(prefers-color-scheme: light)').matches;
  const screen = await render(<General onReference={() => {}} />);
  const theme = screen.getByRole('combobox', { name: 'Theme' });
  await expect.element(theme).toHaveValue('system');

  await userEvent.selectOptions(theme, systemLight ? 'Dark' : 'Light');
  expect(light()).toBe(!systemLight);

  // A new start: the class comes from what was kept, before any screen draws.
  document.documentElement.classList.toggle('light', systemLight);
  followTheme();
  expect(light()).toBe(!systemLight);
  await screen.rerender(<General key="again" onReference={() => {}} />);
  const kept = screen.getByRole('combobox', { name: 'Theme' });
  await expect.element(kept).toHaveValue(systemLight ? 'dark' : 'light');

  await userEvent.selectOptions(kept, 'System');
  expect(light()).toBe(systemLight);
});

test('the desktop window title bar takes Theme at start and on each change, and System hands it back', async () => {
  service(false, [], []);
  const calls: unknown[] = [];
  vi.stubGlobal('__TAURI_INTERNALS__', {
    invoke: async (cmd: string, args: unknown) => calls.push([cmd, args]),
  });
  cookie('dark');
  followTheme();
  const screen = await render(<General onReference={() => {}} />);
  const theme = screen.getByRole('combobox', { name: 'Theme' });
  await userEvent.selectOptions(theme, 'Light');
  await userEvent.selectOptions(theme, 'System');
  const set = (value: string | null) => ['plugin:window|set_theme', { value }];
  expect(calls).toEqual([set('dark'), set('light'), set(null)]);
});

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

test('a read on focus that answers after a newer read never shows the older state', async () => {
  let release = () => {};
  // Mount reads off; the next focus read is taken while off and held; the one after reads on.
  const reads = [
    { on: false, hold: false },
    { on: false, hold: true },
    { on: true, hold: false },
  ];
  vi.stubGlobal('fetch', async (path: string) => {
    if (path === '/api/settings') return new Response(JSON.stringify({ stale_days: 3, cancel_days: 7 }));
    if (path === '/api/app') return new Response('{}');
    const read = reads.shift() ?? { on: true, hold: false };
    const answer = new Response(JSON.stringify({ on: read.on }));
    if (read.hold) await new Promise<void>((done) => (release = done));
    return answer;
  });
  const screen = await render(<General onReference={() => {}} />);
  const toggle = screen.getByRole('switch', { name: 'Open at login' });
  await expect.element(toggle).toHaveAttribute('aria-checked', 'false');

  window.dispatchEvent(new Event('focus'));
  window.dispatchEvent(new Event('focus'));
  await expect.element(toggle).toHaveAttribute('aria-checked', 'true');
  release();
  await new Promise((done) => setTimeout(done, 50));
  await expect.element(toggle).toHaveAttribute('aria-checked', 'true');
});

test('a read on focus that fails while a save is pending leaves no failure once the save succeeds', async () => {
  let release = () => {};
  let holdPut = false;
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/settings') return new Response(JSON.stringify({ stale_days: 3, cancel_days: 7 }));
    if (path === '/api/app') return new Response('{}');
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
