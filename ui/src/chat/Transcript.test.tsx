// Live events recorded from a real gathrd on Claude Code's recorded frames, replayed into the
// transcript as the socket delivers them (the UI's side of the engine-replay tests).
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { LiveEvent } from '../live';
import approval from './recorded/approval.json';
import exitsMidReply from './recorded/exits-mid-reply.json';
import { rows } from './rows';
import { apply, empty } from './state';
import { type NoticeActions, Transcript } from './Transcript';

const none: NoticeActions = { diagnostics: () => {}, signIn: () => {}, retry: () => {}, carryOn: () => {} };

function Replay({ events }: { events: LiveEvent[] }) {
  return <Transcript rows={rows(events.reduce(apply, empty))} actions={none} />;
}

const reply = 'I created `note.txt` in the current folder with the Write tool. It contains the word `hi`.';

test('a reply shows while it streams, and once when it is saved', async () => {
  const events = approval as LiveEvent[];
  const saved = events.findLastIndex((e) => e.type === 'message');
  const screen = await render(<Replay events={events.slice(0, saved)} />);
  await expect.element(screen.getByText(/^I created `note.txt` in the current folder/)).toBeVisible();

  await screen.rerender(<Replay events={events} />);
  await expect.element(screen.getByText(reply, { exact: true })).toBeVisible();
  expect(screen.getByText(reply, { exact: true }).elements()).toHaveLength(1);
  await expect.element(screen.getByText('1 tool call')).toBeVisible();
});

test('an engine that exits mid-reply keeps the text shown and fails with its reference', async () => {
  const screen = await render(<Replay events={exitsMidReply as LiveEvent[]} />);
  await expect.element(screen.getByText('Already visible answer')).toBeVisible();
  await expect.element(screen.getByRole('alert').getByText('Reply failed', { exact: true })).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Reference 5236079d' })).toBeVisible();
});
