// Live events recorded from a real gathrd on Claude Code's recorded frames, replayed into the
// thread as the socket delivers them (the UI's side of the engine-replay tests).
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import { Failure, type Message } from '../api';
import type { LiveEvent } from '../live';
import '../theme.css';
import type { SendFailure } from './messages';
import approval from './recorded/approval.json';
import exitsMidReply from './recorded/exits-mid-reply.json';
import sendFailed from './recorded/send-failed.json';
import { apply, type ChatState, empty } from './state';
import { type Actions, Thread } from './Thread';

const none: Actions = {
  diagnostics: () => {},
  signIn: () => {},
  retry: () => {},
  resend: () => {},
  carryOn: () => {},
  approve: () => {},
};

function View({ state, unsent = [] }: { state: ChatState; unsent?: SendFailure[] }) {
  return (
    <div style={{ height: 600, display: 'flex' }}>
      <Thread state={state} agents={[]} unsent={unsent} running={false} actions={none} empty={false}>
        <p>Composer</p>
      </Thread>
    </div>
  );
}
const replay = (events: unknown) => (events as LiveEvent[]).reduce(apply, empty);

const reply = 'I created note.txt in the current folder with the Write tool. It contains the word hi.';

test('a reply shows while it streams, and once when it is saved', async () => {
  const events = approval as LiveEvent[];
  const saved = events.findLastIndex((e) => e.type === 'message');
  const screen = await render(<View state={replay(events.slice(0, saved))} />);
  await expect.element(screen.getByText(/^I created note.txt in the current folder/)).toBeVisible();
  await screen.rerender(<View state={replay(events)} />);
  await expect.element(screen.getByText(reply, { exact: true })).toBeVisible();
  expect(screen.getByText(reply, { exact: true }).elements()).toHaveLength(1);
  await expect.element(screen.getByText('1 tool call')).toBeVisible();
});

test('a reply is formatted, and the HTML or script links in it do nothing', async () => {
  const text = [
    '## Plan',
    '- **Keep** `main` green, see [the notes](https://example.com/notes)',
    '<img src=x onerror="window.ran=1"> [run](javascript:window.ran=1)',
    '| Step | Time |\n|---|---|\n| Build | 2 s |',
    'Said once.[^1]\n\n[^1]: Where it was said.',
  ].join('\n\n');
  const message: Message = { id: 'm', turn: 't', time: '', role: 'assistant', kind: 'text', body: text };
  const screen = await render(<View state={{ ...empty, messages: [message] }} />);
  await expect.element(screen.getByRole('heading', { name: 'Plan' })).toBeVisible();
  await expect.element(screen.getByRole('table')).toBeVisible();
  await expect.element(screen.getByRole('link', { name: 'the notes' })).toHaveAttribute('target', '_blank');
  await expect.element(screen.getByText(/^<img src=x onerror="window.ran=1">/)).toBeVisible();
  expect(document.querySelector('.reply img')).toBeNull();
  await expect.element(screen.getByText(/run \[blocked\]$/)).toBeVisible();
  expect(document.querySelector('.reply a[href*="javascript"]')).toBeNull();
  // A footnote's link moves within the reply, so it never leaves for the browser.
  for (const link of document.querySelectorAll<HTMLAnchorElement>('.reply a[href^="#"]')) {
    expect(link.target).toBe('');
  }
  expect((window as { ran?: number }).ran).toBeUndefined();
});

test('an engine that exits mid-reply keeps the text shown and fails with its reference', async () => {
  const screen = await render(<View state={replay(exitsMidReply)} />);
  await expect.element(screen.getByText('Already visible answer')).toBeVisible();
  await expect.element(screen.getByRole('alert').getByText('Reply failed', { exact: true })).toBeVisible();
  await expect.element(screen.getByRole('button', { name: 'Reference 5236079d' })).toBeVisible();
});

test('a send that fails says so once, with its reference', async () => {
  const answer = new Failure('Send failed', "Claude Code's path isn't a file", 'a0c5a2a6');
  const unsent = [{ id: '1', text: 'Summarise the release notes.', failure: answer }];
  const screen = await render(<View state={replay(sendFailed)} unsent={unsent} />);
  await expect.element(screen.getByRole('button', { name: 'Reference a0c5a2a6' })).toBeVisible();
  expect(screen.getByRole('alert').elements()).toHaveLength(1);
});

test('a long chat draws its newest messages, and older ones as you scroll up, keeping your place', async () => {
  const messages: Message[] = Array.from({ length: 300 }, (_, i) => ({
    id: `m${i}`,
    turn: null,
    time: '',
    role: 'user',
    kind: 'text',
    body: `Made-up message ${i}`,
  }));
  const screen = await render(<View state={{ ...empty, messages }} />);
  await expect.element(screen.getByText('Made-up message 299', { exact: true })).toBeInTheDocument();
  expect(screen.getByText('Made-up message 199', { exact: true }).query()).toBeNull();

  const viewport = document.querySelector('.thread-viewport') as HTMLElement;
  const first = screen.getByText('Made-up message 200', { exact: true });
  viewport.scrollTop = 0;
  const top = first.element().getBoundingClientRect().top;
  await expect.element(screen.getByText('Made-up message 199', { exact: true })).toBeInTheDocument();
  // The message that was at the top stays where it was on screen.
  await expect.poll(() => Math.abs(first.element().getBoundingClientRect().top - top)).toBeLessThan(2);
});
