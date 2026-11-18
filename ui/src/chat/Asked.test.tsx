// A question answered in the chat (0073): the asker's bubble holds the one question block, with the same draft as
// the Tasks view, and an answer goes once, whichever place sends it. The service is the edge: fetch answers as it
// would. Made-up team and questions.
import { useRef } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Agent, Message, Task } from '../api';
import { asQuestion } from '../tasks/rows';
import { TasksTab } from '../tasks/TasksTab';
import { usePersonTasks } from '../tasks/usePersonTasks';
import { toThread } from './messages';
import { type ChatState, empty } from './state';
import { type Actions, Thread } from './Thread';

vi.mock('../live', () => ({ listen: () => () => {} }));
afterEach(() => vi.unstubAllGlobals());

const members = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' as const },
  { agent: 'ann', name: 'Ann', role: 'member' as const },
];
const agents = members.map((m) => ({ id: m.agent, name: m.name, face_seed: 'Made-up' })) as Agent[];
const name = (a: string | null) => members.find((m) => m.agent === a)?.name ?? 'An agent';
const none = () => {};
const actions: Actions = {
  openTask: none,
  openAgent: none,
  diagnostics: none,
  reference: none,
  signIn: none,
  retry: none,
  resend: none,
  carryOn: none,
  approve: none,
};
const time = '2026-11-08T09:00:00Z';
const reply: Message = {
  id: 'm1',
  turn: 't1',
  time,
  role: 'assistant',
  kind: 'text',
  body: 'Ann asks for this one.',
};
const question = (more: Partial<Task>): Task => ({
  number: 5,
  chat: 'c',
  giver: 'mira',
  owner: null,
  given_in: 't1',
  kind: 'person',
  what: 'A recipe with no photo: keep the placeholder, or use a step’s photo?',
  state: 'ready',
  reason: null,
  after: [],
  form: 'choice',
  choices: [{ label: 'Keep the placeholder' }, { label: 'Use a step’s photo' }],
  created_us: (Date.parse(time) + 100) * 1000,
  ...more,
});
const chat = (tasks: Task[]): ChatState => ({
  ...empty,
  members,
  turns: { t1: { id: 't1', agent: 'mira', status: 'done', why: null, ref: 't1', position: 1 } },
  messages: [reply],
  tasks,
});

/** The chat and the Tasks view of one question, as the app holds them: one draft, one Send. */
function Room({ state }: { state: ChatState }) {
  const person = usePersonTasks(none);
  const cache = useRef(new Map());
  const send = (t: Task, d: Parameters<typeof person.send>[1]) => person.send(asQuestion(t, state.tasks), d);
  return (
    <>
      <section aria-label="Chat" style={{ height: 420, display: 'flex' }}>
        <Thread
          messages={toThread(state, agents, [], cache.current).items}
          running={false}
          actions={actions}
          empty={false}
          questions={{ tasks: state.tasks, drafts: person.drafts, onDraft: person.draft, onSend: send, name }}
        />
      </section>
      <section aria-label="Tasks view">
        <TasksTab
          tasks={state.tasks}
          members={members}
          opened={state.tasks[0].number}
          onOpen={none}
          drafts={person.drafts}
          onDraft={person.draft}
          name={name}
          onOpenFile={none}
          onAnswer={send}
          onSettle={async () => {}}
          onReference={none}
        />
      </section>
    </>
  );
}

/** The service: the person's list is empty, and each answer waits for `release` once posted. */
function service() {
  const posted: string[] = [];
  let release = () => {};
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (path !== '/api/person-tasks/answer') return new Response(JSON.stringify({ tasks: [], stale: [] }));
    posted.push(String(init?.body));
    await new Promise<void>((done) => (release = done));
    return new Response(JSON.stringify({ answered: [], refused: [] }));
  });
  return { posted, release: () => release() };
}

test('a question answered in the chat sends what the Tasks view sends: one block, one draft', async () => {
  const edge = service();
  const screen = await render(<Room state={chat([question({})])} />);
  const inChat = screen.getByRole('region', { name: 'Chat' });
  const inTasks = screen.getByRole('region', { name: 'Tasks view' });
  await inChat.getByRole('button', { name: /Use a step’s photo/ }).click();
  await expect
    .element(inTasks.getByRole('button', { name: /Use a step’s photo/ }))
    .toHaveAttribute('aria-pressed', 'true');
  await inTasks.getByRole('textbox', { name: 'Notes' }).fill('Only for the finished dish.');
  await expect
    .element(inChat.getByRole('textbox', { name: 'Notes' }))
    .toHaveValue('Only for the finished dish.');
  await inChat.getByRole('button', { name: 'Send' }).click();
  await expect
    .poll(() => edge.posted)
    .toEqual(['{"answers":[{"task":5,"pick":["Use a step’s photo"],"text":"Only for the finished dish."}]}']);
  edge.release();
});

test('one question is answered once, though the chat and the Tasks view both offer it', async () => {
  const edge = service();
  const gathrs = question({
    giver: null,
    given_in: null,
    what: 'Let it go on?',
    why: '#18 adds 1,240 lines, over this project’s line of 600.',
    choices: [{ label: 'Let it go on' }, { label: 'Send it back' }],
  });
  const screen = await render(<Room state={chat([gathrs])} />);
  await screen
    .getByRole('region', { name: 'Chat' })
    .getByRole('button', { name: /Send it back/ })
    .click();
  await expect.poll(() => edge.posted.length).toBe(1);
  await screen
    .getByRole('region', { name: 'Tasks view' })
    .getByRole('button', { name: /Let it go on/ })
    .click();
  await new Promise((r) => setTimeout(r, 100));
  edge.release();
  expect(edge.posted).toEqual(['{"answers":[{"task":5,"pick":["Send it back"]}]}']);
});

test('after the answer, the chat shows the person’s message once, below the question, which reads as answered', async () => {
  service();
  const answered = question({
    state: 'done',
    reason: 'Keep the placeholder',
    answer: { pick: ['Keep the placeholder'], at: (Date.parse(time) + 900) * 1000 },
  });
  const screen = await render(<Room state={chat([answered])} />);
  const inChat = screen.getByRole('region', { name: 'Chat' });
  const said = inChat.getByText('Keep the placeholder', { exact: true });
  await expect.element(said).toBeVisible();
  expect(said.all()).toHaveLength(1);
  const asked = inChat.getByText(answered.what);
  expect(said.element().getBoundingClientRect().top).toBeGreaterThan(
    asked.element().getBoundingClientRect().bottom,
  );
  await expect.element(inChat.getByText('Waiting')).not.toBeInTheDocument();
  await expect.element(inChat.getByRole('button', { name: /Use a step’s photo/ })).not.toBeInTheDocument();
});
