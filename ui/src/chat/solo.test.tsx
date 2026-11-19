// A chat of one agent has what a team chat has (0028, 0038): the faces button and Summary, its agent's mark and card
// with its role, a face that opens the agent's panel, and the role in the "@" list. Its name line names no role:
// that line is redesigned apart. The service is the edge: fetch answers as it would.
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Agent, Member, Message, Standing, Turn, Waiting } from '../api';
import type { LiveEvent } from '../live';
import { ChatView } from './ChatView';
import { nameable } from './recipient';
import '../app.css';

vi.mock('../live', () => ({ listen: (_: (event: LiveEvent) => void) => () => {} }));
beforeEach(() => page.viewport(1280, 800));
afterEach(() => vi.unstubAllGlobals());

const mira: Member = { agent: 'm', name: 'Made-up Mira', role: 'supervisor' };
const agent = (id: string, name: string): Agent => ({
  id,
  name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto',
  face_seed: 'Made-up',
  gender: null,
  skin: null,
  background: null,
});
const agents = [agent('m', 'Made-up Mira'), agent('b', 'Made-up Bo')];
const turn = (id: string, position: number, more: Partial<Turn> = {}): Turn => ({
  id,
  agent: 'm',
  status: 'done',
  why: null,
  ref: id,
  position,
  ...more,
});
const said = (id: string, turnId: string, role: Message['role'], body: string): Message => ({
  id,
  turn: turnId,
  time: '2026-11-08T09:00:00Z',
  role,
  kind: 'text',
  body,
});
const history = {
  chat: 'c',
  members: [mira],
  approvals: [],
  turns: [turn('ask', 0), turn('fail', 1, { status: 'failed' })],
  messages: [
    said('u1', 'ask', 'user', 'Made-up question'),
    said('m1', 'ask', 'assistant', 'Made-up answer'),
    said('u2', 'fail', 'user', 'Made-up follow-up'),
    said('m2', 'fail', 'assistant', 'Made-up half a reply'),
  ],
};
const failed: Standing = {
  agent: 'm',
  name: 'Made-up Mira',
  state: 'cant_run',
  words: 'Its last turn failed',
  what: 'Failed',
  task: null,
  since_us: (Date.now() - 125_000) * 1000,
  on: [],
  until_us: null,
  failing: false,
};
const waiting: Waiting = {
  lines: [],
  agents: [failed],
  gathr: [
    {
      kind: 'command',
      state: 'working',
      what: 'Made-up build',
      name: 'Made-up Mira',
      since_us: (Date.now() - 60_000) * 1000,
      failing: false,
    },
  ],
};

function service() {
  vi.stubGlobal('fetch', async (path: string) => {
    if (path.endsWith('/waiting')) return new Response(JSON.stringify(waiting));
    if (path.endsWith('/tasks')) return new Response(JSON.stringify({ tasks: [], waits: [] }));
    return new Response(JSON.stringify(path === '/api/pauses' ? {} : history));
  });
}

function chat(onOpenAgent: (agent: string) => void = () => {}) {
  return render(
    <div className="app">
      <ChatView
        chat={{ id: 'c', title: 'Made-up chat' }}
        agents={agents}
        draft=""
        onDraft={() => {}}
        unsent={[]}
        onUnsent={() => {}}
        openSettings={() => {}}
        onNamed={() => {}}
        projects={[]}
        onProject={() => {}}
        onNewProject={() => {}}
        onStart={() => {}}
        pauses={{ pauses: [], usage: [] }}
        onPause={async () => {}}
        onOpenAgent={onOpenAgent}
      />
    </div>,
  );
}

/** Each agent's face and its mark: "m:cant". */
const faces = (within: string) =>
  [...document.querySelectorAll(`${within} .face[data-agent]`)].map(
    (face) =>
      `${face.getAttribute('data-agent')}:${face.querySelector('.mark')?.getAttribute('data-state') ?? '-'}`,
  );
const rows = () =>
  [...document.querySelectorAll('.agent-row')].map((row) =>
    [...row.children].map((part) => part.textContent).filter(Boolean),
  );

test('a chat of one has the faces button with its one agent; Summary lists it, Gathr’s waits and Pause or resume, with no Round', async () => {
  service();
  const opened: string[] = [];
  const screen = await chat((id) => opened.push(id));
  await expect.poll(() => faces('.topline .roster')).toEqual(['m:cant']);
  const button = screen.getByRole('button', { name: /^Summary/ });
  await button.click();
  await expect.poll(() => rows().map((r) => r[0])).toEqual(['Made-up Mira', 'Gathr']);
  expect(rows()[0][1]).toBe('Its last turn failed');
  expect(rows()[1].join(' ')).toContain('Made-up build');
  await expect.element(page.getByRole('region', { name: 'Round' })).not.toBeInTheDocument();

  await page.getByRole('button', { name: 'Pause or resume' }).click();
  await expect.element(page.getByRole('menuitem', { name: 'Pause Made-up Mira' })).toBeVisible();
  await userEvent.keyboard('{Escape}');

  (document.querySelector('.agent-row') as HTMLElement).click();
  expect(opened).toEqual(['m']);
});

test('the agent’s face on a message has its mark, a card with its role, and opens its panel; its name line names no role', async () => {
  service();
  const opened: string[] = [];
  const screen = await chat((id) => opened.push(id));
  await expect.poll(() => faces('.thread-messages')).toEqual(['m:cant', 'm:cant']);
  const face = screen.getByRole('button', { name: 'Open Made-up Mira' }).first();
  await userEvent.hover(face);
  await expect.element(page.getByText('Supervisor', { exact: true })).toBeVisible();
  await userEvent.unhover(face);
  await face.click();
  expect(opened).toEqual(['m']);
  expect(document.querySelector('.thread-messages')?.textContent).not.toContain('Supervisor');
});

test('the "@" list names the one agent’s role', () => {
  const people = nameable({ members: [mira], agents, project: false, fresh: false, last: 'm' });
  expect(people.map((p) => [p.name, p.meta ?? null])).toEqual([
    ['Made-up Mira', 'Supervisor'],
    ['Made-up Bo', null],
  ]);
});
