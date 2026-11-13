// The service is the edge here: fetch answers as it would, from made-up reports.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import { Friction } from './Friction';

afterEach(() => vi.unstubAllGlobals());

const answers = (friction: unknown[]) =>
  vi.stubGlobal('fetch', async (path: string) =>
    path === '/api/friction' ? new Response(JSON.stringify({ friction })) : new Response('{}'),
  );

test('friction is listed newest first, each with who reported it, on which task and where', async () => {
  answers([
    {
      id: 2,
      time: '2026-11-13T14:02:00.000Z',
      chat: 'c1',
      chat_title: 'Made-up chat',
      agent: 'a1',
      agent_name: 'Made-up Member',
      task: 14,
      what: 'The made-up checks run timed out with no reference',
      where: 'checks',
    },
    {
      id: 1,
      time: '2026-11-13T11:40:00.000Z',
      chat: 'c0',
      chat_title: null,
      agent: null,
      agent_name: null,
      task: null,
      what: 'A made-up message was cut short',
      where: null,
    },
  ]);
  const screen = await render(<Friction />);
  await expect.element(screen.getByRole('heading', { name: 'Friction' })).toBeVisible();
  await expect.element(screen.getByText('The made-up checks run timed out with no reference')).toBeVisible();
  const items = screen.getByRole('listitem').all();
  expect(items).toHaveLength(2);
  const first = items[0].element().textContent ?? '';
  expect(first.startsWith('The made-up checks run timed out with no reference')).toBe(true);
  expect(first).toContain('Made-up Member · Task #14 · Made-up chat · ');
  expect(first.endsWith(' · checks')).toBe(true);
  // A report whose agent and chat are gone still shows, with what is left of it.
  const second = items[1].element().textContent ?? '';
  expect(second.startsWith('A made-up message was cut shortAn agent · ')).toBe(true);
});

test('with nothing reported it says so, and a failure shows with its reference', async () => {
  answers([]);
  const screen = await render(<Friction />);
  await expect.element(screen.getByText('No friction reported yet')).toBeVisible();
  screen.unmount();

  const error = { error: 'Friction failed', why: 'made-up refusal', ref: 'made-up-ref' };
  vi.stubGlobal('fetch', async () => new Response(JSON.stringify(error), { status: 500 }));
  const failed = await render(<Friction />);
  await expect.element(failed.getByText('made-up refusal')).toBeVisible();
});
