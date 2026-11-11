// Search across chats (plans/search.md): one cursor over Chats then Messages, Enter, Escape, the query
// kept between openings, and an answer to an older query dropped.
import { afterEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { SearchFlyout } from './SearchFlyout';

afterEach(() => vi.unstubAllGlobals());

const json = (body: unknown) => new Response(JSON.stringify(body));
const chat = (title: string) => ({ chat: `c-${title}`, title, archived: null, time: '2026-01-05T09:52:00Z' });
const message = (n: number, snippet: string) => ({
  chat: 'c1',
  chat_title: 'Made-up chat',
  archived: n === 2 ? '2026-01-04T00:00:00Z' : null,
  message: `m${n}`,
  role: 'assistant',
  agent: 'a',
  time: '2026-01-05T09:52:00Z',
  snippet,
  marks: [[0, 6]],
});
const answers: Record<string, unknown> = {
  gravel: { chats: [chat('Gravel road')], messages: [message(1, 'Gravel path'), message(2, 'Gravel yard')] },
  gr: { chats: [], messages: [message(3, 'Green field')] },
};

function setup(delays: Record<string, number> = {}) {
  vi.stubGlobal('fetch', async (path: string) => {
    const q = new URL(path, location.href).searchParams.get('q') ?? '';
    await new Promise((r) => setTimeout(r, delays[q] ?? 0));
    return json(answers[q] ?? { chats: [], messages: [] });
  });
  const onOpen = vi.fn();
  render(<SearchFlyout name={() => 'Made-up Agent'} onOpen={onOpen} onReference={() => {}} />);
  return onOpen;
}

const input = () => page.getByRole('combobox', { name: 'Search chats and messages' });
const selected = (name: string) => page.getByRole('option', { name: new RegExp(name), selected: true });

test('one cursor runs across Chats and Messages, and Enter opens the row under it', async () => {
  const onOpen = setup();
  await page.getByRole('button', { name: /Search/ }).click();
  await expect.element(input()).toHaveFocus();
  await userEvent.keyboard('gravel');
  await expect.element(page.getByRole('option')).toHaveLength(3);
  await expect.element(page.getByText('Archived')).toBeVisible();
  // With nothing selected, Up starts at the last row; Down from there wraps to the first.
  await userEvent.keyboard('{ArrowUp}');
  await expect.element(selected('Gravel yard')).toBeVisible();
  await userEvent.keyboard('{ArrowDown}');
  await expect.element(selected('Gravel road')).toBeVisible();
  await userEvent.keyboard('{ArrowDown}');
  await expect.element(selected('Gravel path')).toBeVisible();
  // Up from the first row wraps to the last.
  await userEvent.keyboard('{ArrowUp}{ArrowUp}');
  await expect.element(selected('Gravel yard')).toBeVisible();
  await userEvent.keyboard('{Enter}');
  expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ message: 'm2' }), expect.any(Number));
  await expect.element(input()).not.toBeInTheDocument();
});

test('Escape closes it, and Ctrl K opens it again with the query kept', async () => {
  setup();
  await page.getByRole('button', { name: /Search/ }).click();
  await userEvent.keyboard('gravel');
  await expect.element(page.getByRole('option')).toHaveLength(3);
  await userEvent.keyboard('{Escape}');
  await expect.element(input()).not.toBeInTheDocument();
  await userEvent.keyboard('{Control>}k{/Control}');
  await expect.element(input()).toHaveFocus();
  await expect.element(input()).toHaveValue('gravel');
  await expect.element(page.getByRole('option')).toHaveLength(3);
});

test('an answer to an older query never replaces a newer one', async () => {
  // "gr" was asked first but answers last.
  setup({ gr: 600 });
  await page.getByRole('button', { name: /Search/ }).click();
  await userEvent.keyboard('gr');
  await new Promise((r) => setTimeout(r, 250));
  await userEvent.keyboard('avel');
  await expect.element(page.getByRole('option')).toHaveLength(3);
  await new Promise((r) => setTimeout(r, 700));
  await expect.element(page.getByRole('option')).toHaveLength(3);
  await expect.element(page.getByText('Green field')).not.toBeInTheDocument();
});
