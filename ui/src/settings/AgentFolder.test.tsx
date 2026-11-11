// The service is the edge here: fetch answers as it would, from made-up state.
import { afterEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { AgentFolder } from './AgentFolder';
import { SettingsPanel } from './SettingsPanel';

const settings = { stale_days: 3, cancel_days: 7, agent_folder: 'X:\\Made\\Up' };

/** Answers GET /api/settings; each PATCH is refused for a folder named `bad`, else saved. */
function service(sent: unknown[]) {
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path === '/api/startup') return new Response(JSON.stringify({ on: false }));
    if (init?.method !== 'PATCH') return new Response(JSON.stringify(settings));
    const change = JSON.parse(String(init.body));
    sent.push(change);
    if (String(change.agent_folder).endsWith('bad')) {
      const body = { error: 'Change failed', why: 'made-up refusal', ref: 'made-up-ref' };
      return new Response(JSON.stringify(body), { status: 400 });
    }
    return new Response(JSON.stringify({ ...settings, ...change }));
  });
}
afterEach(() => vi.unstubAllGlobals());

test('a refused folder keeps what was typed with its reference; a good one saves and shows', async () => {
  const sent: unknown[] = [];
  service(sent);
  const opened: string[] = [];
  const screen = await render(<AgentFolder onReference={(ref) => opened.push(ref)} />);
  await expect.element(screen.getByText('X:\\Made\\Up')).toBeVisible();

  await screen.getByRole('button', { name: 'Change' }).click();
  const field = screen.getByRole('textbox', { name: 'Agent folder' });
  await expect.element(field).toHaveValue('X:\\Made\\Up');
  await expect.element(field).toHaveFocus();
  await expect
    .element(field)
    .toHaveAccessibleDescription('Applies to new work. Current work stays where it is.');

  await userEvent.fill(field, 'Y:\\bad');
  await userEvent.keyboard('{Enter}');
  await expect.element(screen.getByText('Change failed')).toBeVisible();
  await screen.getByRole('button', { name: 'Reference made-up-ref' }).click();
  expect(opened).toEqual(['made-up-ref']);
  await expect.element(field).toHaveValue('Y:\\bad');

  await userEvent.fill(field, 'Y:\\Agents');
  await screen.getByRole('button', { name: 'Save' }).click();
  await expect.element(screen.getByText('Y:\\Agents')).toBeVisible();
  await expect.element(screen.getByText('Change failed')).not.toBeInTheDocument();
  await expect.element(screen.getByRole('button', { name: 'Change' })).toHaveFocus();
  expect(sent).toEqual([{ agent_folder: 'Y:\\bad' }, { agent_folder: 'Y:\\Agents' }]);
});

test('Escape cancels the edit without closing Settings, and saves nothing', async () => {
  const sent: unknown[] = [];
  service(sent);
  let closed = 0;
  const screen = await render(
    <SettingsPanel
      target={{ page: 'general', opener: null }}
      onPage={() => {}}
      onReference={() => {}}
      onClose={() => closed++}
    />,
  );
  await screen.getByRole('button', { name: 'Change' }).click();
  await userEvent.fill(screen.getByRole('textbox', { name: 'Agent folder' }), 'Y:\\Other');
  await userEvent.keyboard('{Escape}');
  await expect.element(screen.getByText('X:\\Made\\Up')).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Change' })).toHaveFocus();
  expect(closed).toBe(0);
  expect(sent).toEqual([]);
});

test('in the desktop window Change opens the system picker: a cancel saves nothing, a pick saves, a failure gives the field', async () => {
  const sent: unknown[] = [];
  service(sent);
  const picks: (string | null | Error)[] = [
    null,
    'Y:\\bad',
    'Y:\\Picked',
    new Error('made-up picker failure'),
  ];
  const asked: unknown[] = [];
  vi.stubGlobal('__TAURI_INTERNALS__', {
    invoke: async (cmd: string, args: unknown) => {
      asked.push([cmd, args]);
      const next = picks.shift();
      if (next instanceof Error) throw next;
      return next;
    },
  });
  const screen = await render(<AgentFolder onReference={() => {}} />);
  const change = screen.getByRole('button', { name: 'Change' });

  // A click that leaves focus where it was, as WebKit's click on a button does.
  await expect.element(change).toBeVisible();
  (change.element() as HTMLElement).click();
  await expect.element(change).toHaveFocus();
  expect(sent).toEqual([]);

  await change.click();
  await expect.element(screen.getByText('Change failed')).toBeVisible();
  await expect.element(screen.getByText('X:\\Made\\Up')).toBeVisible();

  await change.click();
  await expect.element(screen.getByText('Y:\\Picked')).toBeVisible();
  await expect.element(screen.getByText('Change failed')).not.toBeInTheDocument();
  expect(sent).toEqual([{ agent_folder: 'Y:\\bad' }, { agent_folder: 'Y:\\Picked' }]);

  await change.click();
  await expect.element(screen.getByRole('textbox', { name: 'Agent folder' })).toHaveValue('Y:\\Picked');
  const options = { directory: true, title: 'Agent folder', canCreateDirectories: true };
  expect(asked[0]).toEqual(['plugin:dialog|open', { options: { ...options, defaultPath: 'X:\\Made\\Up' } }]);
  expect(asked[3]).toEqual(['plugin:dialog|open', { options: { ...options, defaultPath: 'Y:\\Picked' } }]);
});

test('Change while the picker is still open opens no second one', async () => {
  service([]);
  let answer = (_: string | null) => {};
  let opened = 0;
  vi.stubGlobal('__TAURI_INTERNALS__', {
    invoke: () => {
      opened++;
      return new Promise((done) => {
        answer = done;
      });
    },
  });
  const screen = await render(<AgentFolder onReference={() => {}} />);
  const change = screen.getByRole('button', { name: 'Change' });
  await change.click();
  await change.click();
  expect(opened).toBe(1);
  answer(null);
  await new Promise((done) => setTimeout(done, 50));
  await change.click();
  expect(opened).toBe(2);
});
