// The names picked from the "@" list stay blue in the person's sent message (0072): sent with their places, drawn
// from the live message and from history, kept with a message that failed to send. A name only typed stays plain.
import { useState } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Message } from '../api';
import type { Picked } from '../components/mention';
import { ChatView } from './ChatView';
import type { SendFailure } from './messages';

/** The live socket, which the test speaks for. */
class Live {
  static now: Live | undefined;
  onopen?: () => void;
  onmessage?: (frame: { data: string }) => void;
  onclose?: () => void;
  constructor() {
    Live.now = this;
  }
  close() {}
}
vi.stubGlobal('WebSocket', Live);

const agent = (id: string, name: string) => ({
  id,
  name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto' as const,
  face_seed: name,
  gender: null,
  skin: null,
  background: null,
});
const team = [
  { agent: 'a', name: 'Made-up A', role: 'supervisor' as const },
  { agent: 'b', name: 'Made-up B', role: 'member' as const },
];
const project = {
  id: 'p',
  name: 'Made-up project',
  folder: 'x',
  members: team,
  checks: [],
  ci: 'gathr' as const,
  merge: 'fast-forward' as const,
};

const said = (id: string, body: string, picks?: Message['picks']): Message => ({
  id,
  turn: null,
  time: '2026-11-18T10:00:00Z',
  role: 'user',
  kind: 'text',
  body,
  picks,
});

/** A service with `messages` in the chat's history; a send gets `answer`. Returns each send's body. */
function service(messages: Message[], answer = () => new Response('{"turn":"t","status":"running"}')) {
  const sent: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.endsWith('/messages')) {
      sent.push(JSON.parse(String(init.body)));
      return answer();
    }
    return new Response(JSON.stringify({ chat: 'c', messages, turns: [], approvals: [], members: team }));
  });
  return sent;
}
afterEach(() => vi.unstubAllGlobals());

function Open() {
  const [draft, setDraft] = useState({ text: '', picks: [] as Picked[] });
  const [unsent, setUnsent] = useState<SendFailure[]>([]);
  return (
    <ChatView
      chat={{ id: 'c', title: 'Made-up chat', project: 'p' }}
      agents={[agent('a', 'Made-up A'), agent('b', 'Made-up B'), agent('d', 'Ann')]}
      draft={draft.text}
      picks={draft.picks}
      onDraft={(text, picks = []) => setDraft({ text, picks })}
      unsent={unsent}
      onUnsent={(update) => setUnsent(update)}
      openSettings={() => {}}
      onNamed={() => {}}
      projects={[project]}
      onProject={() => {}}
      onNewProject={() => {}}
      onStart={() => {}}
      pauses={{ pauses: [], usage: [] }}
      onPause={async () => {}}
    />
  );
}

/** Each of the person's bubbles, in order: its blue names. */
const blueNames = () =>
  [...document.querySelectorAll('.said[data-you] .bubble')].map((b) =>
    [...b.querySelectorAll('.picked-name')].map((n) => n.textContent),
  );
/** The colour the message box draws a picked name in. */
function boxBlue() {
  const probe = document.createElement('span');
  probe.style.color = 'var(--attention)';
  document.body.append(probe);
  const color = getComputedStyle(probe).color;
  probe.remove();
  return color;
}

async function typeWithPick() {
  const input = page.getByRole('textbox', { name: 'Message' });
  // The keyboard types an emoji as two broken halves, so it comes in by fill.
  await input.fill('Hi 😀 ');
  await userEvent.keyboard('@made-up');
  await expect.element(page.getByRole('listbox', { name: 'Agents' })).toBeVisible();
  await userEvent.keyboard('{ArrowDown}{Enter}and @Ann see');
  await expect.element(input).toHaveValue('Hi 😀 @Made-up B and @Ann see');
}

test('a picked name is sent with its place and is blue in the sent message, live; a typed @Ann stays plain', async () => {
  const sent = service([]);
  await render(<Open />);
  await typeWithPick();
  await userEvent.keyboard('{Enter}');
  const picks = [{ at: 6, len: 10, agent: 'b' }];
  await expect.poll(() => sent).toEqual([{ text: 'Hi 😀 @Made-up B and @Ann see', agent: 'b', picks }]);

  const message = { ...said('m1', 'Hi 😀 @Made-up B and @Ann see', picks), turn: 't', agent: 'b' };
  Live.now?.onmessage?.({ data: JSON.stringify({ type: 'message', chat: 'c', message }) });
  await expect.poll(blueNames).toEqual([['@Made-up B']]);
  const name = document.querySelector('.said[data-you] .picked-name') as HTMLElement;
  expect(getComputedStyle(name).color).toBe(boxBlue());
});

test('from history: two picks blue, a removed agent’s pick blue, a message from before picks plain', async () => {
  service([
    said('m1', '@Made-up A and @Made-up B, é @Ann typed', [
      { at: 0, len: 10, agent: 'a' },
      { at: 15, len: 10, agent: 'b' },
    ]),
    said('m2', '@Made-up B from before', null),
    said('m3', 'ü 😀 @Gone check', [{ at: 5, len: 5, agent: 'gone' }]),
  ]);
  await render(<Open />);
  await expect.poll(blueNames).toEqual([['@Made-up A', '@Made-up B'], [], ['@Gone']]);
  await expect.element(page.getByText('ü 😀 @Gone check')).toBeVisible();
});

test('a message that failed to send keeps its picks blue, and Retry sends them again', async () => {
  let fail = true;
  const refused = () => new Response('{"error":"Send failed","why":"made up","ref":"r1"}', { status: 500 });
  const sent = service([], () => (fail ? refused() : new Response('{"turn":"t","status":"running"}')));
  await render(<Open />);
  await typeWithPick();
  await userEvent.keyboard('{Enter}');
  await expect.poll(blueNames).toEqual([['@Made-up B']]);
  fail = false;
  await page.getByRole('button', { name: 'Retry' }).click();
  const body = { text: 'Hi 😀 @Made-up B and @Ann see', agent: 'b', picks: [{ at: 6, len: 10, agent: 'b' }] };
  await expect.poll(() => sent).toEqual([body, body]);
});
