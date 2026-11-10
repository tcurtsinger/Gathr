// Live events recorded from a real gathrd on Claude Code's recorded frames, replayed into the
// thread as the socket delivers them (the UI's side of the engine-replay tests).
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { type Agent, Failure, type History, type Message, type Turn } from '../api';
import type { LiveEvent } from '../live';
import '../theme.css';
import type { SendFailure } from './messages';
import approval from './recorded/approval.json';
import exitsMidReply from './recorded/exits-mid-reply.json';
import sendFailed from './recorded/send-failed.json';
import { apply, type ChatState, empty, fromHistory } from './state';
import { type Actions, Thread } from './Thread';

const none: Actions = {
  diagnostics: () => {},
  reference: () => {},
  signIn: () => {},
  retry: () => {},
  resend: () => {},
  carryOn: () => {},
  approve: () => {},
};

function View({
  state,
  unsent = [],
  agents = [],
}: {
  state: ChatState;
  unsent?: SendFailure[];
  agents?: Agent[];
}) {
  return (
    <div style={{ height: 600, display: 'flex' }}>
      <Thread state={state} agents={agents} unsent={unsent} running={false} actions={none} empty={false}>
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

const said = (id: string, role: 'user' | 'assistant', body: string): Message => ({
  id,
  turn: 't',
  time: '',
  role,
  kind: 'text',
  body,
});
const turn = (status: Turn['status']): Record<string, Turn> => ({
  t: { id: 't', status, why: null, ref: 'r1', agent: 'a1' },
});
const order = () => [...document.querySelectorAll('.user-bubble, .reply-body')].map((e) => e.textContent);

test('a message sent into a running turn keeps the reply before it, and the next reply comes after it', async () => {
  const before = [
    said('u1', 'user', 'First ask'),
    said('r1', 'assistant', 'Hi!'),
    said('u2', 'user', 'Also this'),
  ];
  const running = { ...empty, messages: before, turns: turn('running'), partial: { t: 'Second rep' } };
  const screen = await render(<View state={running} />);
  await expect.element(screen.getByText('Second rep')).toBeVisible();
  expect(order()).toEqual(['First ask', 'Hi!', 'Also this', 'Second rep']);

  const saved = {
    ...empty,
    messages: [...before, said('r2', 'assistant', 'Second reply')],
    turns: turn('done'),
  };
  await screen.rerender(<View state={saved} />);
  await expect.element(screen.getByText('Second reply')).toBeVisible();
  expect(order()).toEqual(['First ask', 'Hi!', 'Also this', 'Second reply']);
});

test("a reply shows its agent's name once the agents arrive, and the new one after a rename", async () => {
  const state = {
    ...empty,
    messages: [said('u1', 'user', 'Ask'), said('r1', 'assistant', 'Done.')],
    turns: turn('done'),
  };
  const agent: Agent = { id: 'a1', name: 'Mira', account: 'x', model: null, effort: null, approvals: 'auto' };
  const screen = await render(<View state={state} />);
  await expect.element(screen.getByText('Done.')).toBeVisible();
  await screen.rerender(<View state={state} agents={[agent]} />);
  await expect.element(screen.getByText('Mira', { exact: true })).toBeVisible();
  await screen.rerender(<View state={state} agents={[{ ...agent, name: 'Rowan' }]} />);
  await expect.element(screen.getByText('Rowan', { exact: true })).toBeVisible();
});

test('a Stop that fails shows its notice alone, with nothing to resend', async () => {
  const failure = { id: 'stop', text: '', failure: new Failure('Connection failed', 'aborted', null) };
  const screen = await render(
    <View state={{ ...empty, messages: [said('u1', 'user', 'Ask')] }} unsent={[failure]} />,
  );
  await expect.element(screen.getByText('Connection failed')).toBeVisible();
  expect(screen.getByRole('button', { name: 'Retry' }).query()).toBeNull();
  expect(document.querySelectorAll('.user-bubble')).toHaveLength(1);
});

const made = (n: number): Message[] =>
  Array.from({ length: n }, (_, i) => ({
    id: `m${i}`,
    turn: null,
    time: '',
    role: 'user',
    kind: 'text',
    body: `Made-up message ${i}`,
  }));

test('the chat fades only at an edge with more of it beyond', async () => {
  const fades = () => ['above', 'below'].filter((e) => viewport.hasAttribute(`data-fade-${e}`)).join(' ');
  const screen = await render(<View state={{ ...empty, messages: made(1) }} />);
  const viewport = document.querySelector('.thread-viewport') as HTMLElement;
  await expect.element(screen.getByText('Made-up message 0', { exact: true })).toBeVisible();
  expect(fades()).toBe('');

  await screen.rerender(<View state={{ ...empty, messages: made(40) }} />);
  viewport.scrollTop = viewport.scrollHeight;
  await expect.poll(fades).toBe('above');
  // The wheel, as a person scrolls: it stops assistant-ui following the bottom. A scrollTop jump didn't
  // on WebKit, where the newly drawn messages' resize came before the scroll and it followed back down.
  await userEvent.wheel(viewport, { delta: { y: -600 } });
  await expect.poll(fades).toBe('above below');
  await userEvent.wheel(viewport, { delta: { y: -100_000 } });
  await expect.poll(fades).toBe('below');
});

test('a long chat draws its newest messages, and older ones as you scroll up, keeping your place', async () => {
  const messages = made(300);
  const screen = await render(<View state={{ ...empty, messages }} />);
  await expect.element(screen.getByText('Made-up message 299', { exact: true })).toBeInTheDocument();
  expect(screen.getByText('Made-up message 199', { exact: true }).query()).toBeNull();

  const viewport = document.querySelector('.thread-viewport') as HTMLElement;
  // WebKit has no scroll anchoring of its own: without it here too, Edge takes WebKit's path.
  viewport.style.overflowAnchor = 'none';
  const first = screen.getByText('Made-up message 200', { exact: true });
  viewport.scrollTop = 0;
  const top = first.element().getBoundingClientRect().top;
  await expect.element(screen.getByText('Made-up message 199', { exact: true })).toBeInTheDocument();
  await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
  // The message that was at the top stays where it was on screen.
  expect(Math.abs(first.element().getBoundingClientRect().top - top)).toBeLessThan(2);
  expect(screen.getByText('Made-up message 99', { exact: true }).query()).toBeNull();
});

test('a wait check approval keeps its exact command through live updates and reload', async () => {
  const command = 'printf allowed > ../outside-engine-folder.txt';
  const before = { ...empty, messages: [said('u', 'user', 'Wait for it')], turns: turn('running') };
  const event: LiveEvent = {
    type: 'approval',
    chat: 'c',
    turn: 't',
    request_id: 'check-1',
    tool: 'wait check',
    input: { command },
  };
  const history: History = {
    chat: 'c',
    messages: before.messages,
    turns: Object.values(before.turns),
    members: [],
    approvals: [{ turn: 't', request_id: 'check-1', tool: 'wait check', input: { command } }],
  };
  const screen = await render(<View state={apply(before, event)} />);
  for (const state of [apply(before, event), fromHistory(history)]) {
    await screen.rerender(<View state={state} />);
    const card = screen.getByRole('group', { name: 'Allow wait check?' });
    await expect.element(card.getByText(command, { exact: true })).toBeVisible();
    await expect.element(card.getByRole('button', { name: 'Allow', exact: true })).toBeEnabled();
    await expect.element(card.getByRole('button', { name: 'Deny', exact: true })).toBeEnabled();
  }
});
