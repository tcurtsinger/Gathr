// No service answers here, so every request fails, as when the service is down.
import { useState } from 'react';
import { expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { Picked } from '../components/mention';
import { ChatView } from './ChatView';
import type { SendFailure } from './messages';

/** Two chats and what the app keeps for each while another is open. */
function TwoChats() {
  const [open, setOpen] = useState('a');
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [unsent, setUnsent] = useState<Record<string, SendFailure[]>>({});
  return (
    <>
      <button type="button" onClick={() => setOpen('a')}>
        Chat A
      </button>
      <button type="button" onClick={() => setOpen('b')}>
        Chat B
      </button>
      <ChatView
        key={open}
        chat={{ id: open, title: `Chat ${open}` }}
        agents={[]}
        draft={drafts[open] ?? ''}
        onDraft={(text) => setDrafts((d) => ({ ...d, [open]: text }))}
        unsent={unsent[open] ?? []}
        onUnsent={(update) => setUnsent((u) => ({ ...u, [open]: update(u[open] ?? []) }))}
        openSettings={() => {}}
        onNamed={() => {}}
        projects={[]}
        onProject={() => {}}
        onNewProject={() => {}}
        onStart={() => {}}
        pauses={{ pauses: [], usage: [] }}
        onPause={async () => {}}
      />
    </>
  );
}

test('a message that never reached the service is still there, with Retry, after switching chats', async () => {
  const screen = await render(<TwoChats />);
  await screen.getByRole('textbox', { name: 'Message' }).fill('Keep this fabricated draft');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect.element(screen.getByText('Keep this fabricated draft', { exact: true })).toBeVisible();

  await screen.getByRole('button', { name: 'Chat B' }).click();
  await expect
    .element(screen.getByText('Keep this fabricated draft', { exact: true }))
    .not.toBeInTheDocument();
  await screen.getByRole('button', { name: 'Chat A' }).click();

  const unsent = screen.getByText('Keep this fabricated draft', { exact: true });
  await expect.element(unsent).toBeVisible();
  await expect.element(screen.getByRole('alert').filter({ hasText: 'Request failed' }).last()).toBeVisible();
});

const made = (id: string) => ({
  id,
  name: `Made-up ${id.toUpperCase()}`,
  account: 'a',
  model: null,
  effort: null,
  approvals: 'auto' as const,
  face_seed: 'Made-up',
  gender: null,
  skin: null,
  background: null,
});
const team = [
  { agent: 'b', name: 'Made-up B', role: 'member' as const },
  { agent: 'a', name: 'Made-up A', role: 'supervisor' as const },
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

/**
 * A service that answers the chat's history: its members and a done turn on `last`, if any. Returns what was sent,
 * each message's body as JSON.
 */
function service(members: typeof team | [], last?: string) {
  const sent: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.endsWith('/messages')) sent.push(JSON.parse(String(init.body)));
    const turns = last ? [{ id: 't', status: 'done', why: null, ref: 'r', agent: last }] : [];
    return new Response(JSON.stringify({ chat: 'c', messages: [], turns, approvals: [], members }));
  });
  return sent;
}

/** A chat in the window, its draft and the names picked in it kept as the app keeps them, each chat's apart. */
interface Shown {
  id?: string;
  project?: string;
  draft?: string;
  started?: unknown[];
}

function Open({ id = 'c', project: p, draft = '', started = [] }: Shown) {
  const [drafts, setDrafts] = useState<Record<string, { text: string; picks: Picked[] }>>({
    [id]: { text: draft, picks: [] },
  });
  const [open, setOpen] = useState(id);
  const here = drafts[open] ?? { text: '', picks: [] };
  return (
    <>
      <button type="button" onClick={() => setOpen(open === id ? 'other' : id)}>
        Switch
      </button>
      <ChatView
        key={open}
        chat={{ id: open, title: open ? 'Made-up chat' : 'New chat', project: p }}
        agents={[made('a'), made('b'), made('c'), { ...made('d'), name: 'Ann' }]}
        draft={here.text}
        picks={here.picks}
        onDraft={(text, picks = []) => setDrafts((d) => ({ ...d, [open]: { text, picks } }))}
        unsent={[]}
        onUnsent={() => {}}
        openSettings={() => {}}
        onNamed={() => {}}
        projects={[project]}
        onProject={() => {}}
        onNewProject={() => {}}
        onStart={(text, agent, picks) => started.push({ text, agent, picks })}
        pauses={{ pauses: [], usage: [] }}
        onPause={async () => {}}
      />
    </>
  );
}

const input = () => page.getByRole('textbox', { name: 'Message' });
const sign = (text: string) => page.getByRole('status').filter({ hasText: text });
const blue = () => [...(CSS.highlights.get('composer-mention') ?? [])].map((r) => (r as Range).toString());
/** Types `before`, then "@" and `query`, and picks the list's row `down` rows below the first. */
async function pick(before: string, query: string, down = 0) {
  await input().click();
  await userEvent.keyboard(`${before}@${query}`);
  await expect.element(page.getByRole('listbox', { name: 'Agents' })).toBeVisible();
  await userEvent.keyboard(`${'{ArrowDown}'.repeat(down)}{Enter}`);
}

test('the hint names who gets a message with no name: the lead, the chat’s own agent, a new chat’s first', async () => {
  service(team, 'b');
  const screen = await render(<Open project="p" />);
  await expect.element(input()).toHaveAttribute('placeholder', 'Message Made-up A');
  service([], 'b');
  await screen.rerender(<Open key="plain" />);
  await expect.element(input()).toHaveAttribute('placeholder', 'Message Made-up B');
  service([]);
  await screen.rerender(<Open key="fresh" id="" />);
  await expect.element(input()).toHaveAttribute('placeholder', 'Message Made-up A');
  vi.unstubAllGlobals();
});

test('a message with no name goes to the agent the hint names, as before', async () => {
  const sent = service([], 'b');
  await render(<Open draft="Made-up question" />);
  await expect.element(input()).toHaveAttribute('placeholder', 'Message Made-up B');
  await page.getByRole('button', { name: 'Send' }).click();
  await expect.poll(() => sent).toEqual([{ text: 'Made-up question', agent: 'b' }]);
  vi.unstubAllGlobals();
});

test('a typed @Name is plain text: not blue, By Send names the lead, and the lead’s id is sent', async () => {
  const sent = service(team);
  await render(<Open project="p" />);
  await input().click();
  await userEvent.keyboard('Thanks, @made-up b can you look');
  await expect.element(sign('To Made-up A')).toBeVisible();
  expect(blue()).toEqual([]);
  await userEvent.keyboard('{Enter}');
  await expect.poll(() => sent).toEqual([{ text: 'Thanks, @made-up b can you look', agent: 'a' }]);
  vi.unstubAllGlobals();
});

test('a name picked from the list is blue, By Send says To it, and its id is sent, in a team chat too', async () => {
  const sent = service(team);
  await render(<Open project="p" />);
  await pick('Thanks, ', 'made-up', 1);
  await userEvent.keyboard('can you look');
  await expect.element(sign('To Made-up B')).toBeVisible();
  expect(blue()).toEqual(['@Made-up B']);
  await userEvent.keyboard('{Enter}');
  await expect
    .poll(() => sent)
    .toEqual([
      { text: 'Thanks, @Made-up B can you look', agent: 'b', picks: [{ at: 8, len: 10, agent: 'b' }] },
    ]);
  vi.unstubAllGlobals();
});

test('an edit inside a picked name makes it plain text, and By Send goes back to the lead', async () => {
  service(team);
  await render(<Open project="p" />);
  await pick('', 'made-up', 1);
  await userEvent.keyboard('look');
  await expect.element(sign('To Made-up B')).toBeVisible();
  // Into the name: "@Made-up B look" with the caret after "Made".
  (input().element() as HTMLTextAreaElement).setSelectionRange(5, 5);
  await userEvent.keyboard('x');
  await expect.element(sign('To Made-up A')).toBeVisible();
  expect(blue()).toEqual([]);
  vi.unstubAllGlobals();
});

test('text typed before a picked name leaves it picked and blue, where it now is', async () => {
  service(team);
  await render(<Open project="p" />);
  await pick('', 'made-up', 1);
  (input().element() as HTMLTextAreaElement).setSelectionRange(0, 0);
  await userEvent.keyboard('Hi ');
  await expect.element(input()).toHaveValue('Hi @Made-up B ');
  await expect.element(sign('To Made-up B')).toBeVisible();
  const [range] = [...(CSS.highlights.get('composer-mention') ?? [])] as Range[];
  expect([range?.startOffset, range?.toString()]).toEqual([3, '@Made-up B']);
  vi.unstubAllGlobals();
});

test('a draft kept while another chat is open keeps its picked name', async () => {
  service(team);
  await render(<Open project="p" />);
  await pick('Ask ', 'made-up', 1);
  await page.getByRole('button', { name: 'Switch' }).click();
  await expect.element(input()).toHaveValue('');
  await page.getByRole('button', { name: 'Switch' }).click();
  await expect.element(input()).toHaveValue('Ask @Made-up B ');
  await expect.element(sign('To Made-up B')).toBeVisible();
  expect(blue()).toEqual(['@Made-up B']);
  vi.unstubAllGlobals();
});

test('an email address names nobody, though its domain is an agent’s name', async () => {
  const sent = service([], 'a');
  await render(<Open />);
  await input().click();
  await userEvent.keyboard('write to bob@ann.com');
  await expect.element(sign('To Made-up A')).toBeVisible();
  await userEvent.keyboard('{Enter}');
  await expect.poll(() => sent).toEqual([{ text: 'write to bob@ann.com', agent: 'a' }]);
  vi.unstubAllGlobals();
});

test('in a chat with no project, picking an agent not in it sends its id, so it joins; By Send says it joins', async () => {
  const sent = service([], 'a');
  await render(<Open />);
  await input().click();
  await userEvent.keyboard('@');
  // The chat's own agent first, then the others under Bring in.
  const options = page.getByRole('listbox', { name: 'Agents' }).getByRole('option');
  await expect
    .poll(() => options.elements().map((o) => o.textContent))
    .toEqual(['Made-up A', 'Made-up B', 'Made-up C', 'Ann']);
  await expect
    .element(page.getByRole('group', { name: 'Bring in' }))
    .toHaveTextContent('Bring inMade-up BMade-up CAnn');
  await userEvent.keyboard('{ArrowUp}{ArrowUp}{Enter}please help');
  await expect.element(sign('Made-up C joins')).toBeVisible();
  await userEvent.keyboard('{Enter}');
  await expect
    .poll(() => sent)
    .toEqual([{ text: '@Made-up C please help', agent: 'c', picks: [{ at: 0, len: 10, agent: 'c' }] }]);
  vi.unstubAllGlobals();
});

test('a new chat’s first message goes to the agent picked in it, and By Send says so', async () => {
  const started: unknown[] = [];
  await render(<Open id="" started={started} />);
  await pick('', 'made-up', 2);
  await userEvent.keyboard('what is left?');
  await expect.element(sign('To Made-up C')).toBeVisible();
  await userEvent.keyboard('{Enter}');
  expect(started).toEqual([
    { text: '@Made-up C what is left?', agent: 'c', picks: [{ at: 0, len: 10, agent: 'c' }] },
  ]);
});

test('Retry sends a failed turn to the agent it was sent to', async () => {
  const sent: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.endsWith('/messages')) {
      sent.push(JSON.parse(String(init.body)));
      return new Response(JSON.stringify({ turn: 'u', status: 'running' }));
    }
    const asked = { id: 'm', turn: 't', time: '', role: 'user', kind: 'text', body: 'Made-up ask' };
    const turns = [{ id: 't', status: 'failed', why: 'made-up', ref: 'r', agent: 'a' }];
    return new Response(
      JSON.stringify({ chat: 'c', messages: [asked], turns, approvals: [], members: team }),
    );
  });
  await render(<Open />);
  await page.getByRole('button', { name: 'Retry' }).click();
  await expect.poll(() => sent).toEqual([{ text: 'Made-up ask', agent: 'a' }]);
  vi.unstubAllGlobals();
});

test('Continue under a member’s stopped turn puts its @Name in the box, so it goes to that member', async () => {
  const sent: unknown[] = [];
  vi.stubGlobal('fetch', async (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.endsWith('/messages')) sent.push(JSON.parse(String(init.body)));
    const said = { id: 'm', turn: 't', time: '', role: 'assistant', kind: 'text', body: 'Made-up half' };
    const turns = [{ id: 't', status: 'stopped', why: null, end: 'you', ref: 'r', agent: 'b' }];
    return new Response(JSON.stringify({ chat: 'c', messages: [said], turns, approvals: [], members: team }));
  });
  await render(<Open project="p" />);
  await page.getByRole('button', { name: 'Continue' }).click();
  await expect.element(input()).toHaveValue('@Made-up B Continue');
  await expect.element(sign('To Made-up B')).toBeVisible();
  await userEvent.keyboard('{Enter}');
  await expect
    .poll(() => sent)
    .toEqual([{ text: '@Made-up B Continue', agent: 'b', picks: [{ at: 0, len: 10, agent: 'b' }] }]);
  vi.unstubAllGlobals();
});

test('a new chat is made with its first message, in its project, to its Supervisor', async () => {
  const started: unknown[] = [];
  const screen = await render(
    <ChatView
      chat={{ id: '', title: 'New chat', project: 'p' }}
      agents={[made('a'), made('b'), made('c')]}
      draft="Made-up first ask"
      onDraft={() => {}}
      unsent={[]}
      onUnsent={() => {}}
      openSettings={() => {}}
      onNamed={() => {}}
      projects={[project]}
      onProject={() => {}}
      onNewProject={() => {}}
      onStart={(text, agent) => started.push({ text, agent })}
      pauses={{ pauses: [], usage: [] }}
      onPause={async () => {}}
    />,
  );
  await expect.element(screen.getByRole('combobox', { name: 'Project' })).toHaveValue('p');
  await expect.element(input()).toHaveAttribute('placeholder', 'Message Made-up A');
  await screen.getByRole('button', { name: 'Send' }).click();
  expect(started).toEqual([{ text: 'Made-up first ask', agent: 'a' }]);
});

test('an approval stays until the service takes the answer; a refused answer shows on it, with its reference', async () => {
  const answers: { allow: boolean; settle: (r: Response) => void }[] = [];
  vi.stubGlobal('fetch', (path: string, init?: RequestInit) => {
    if (init?.method === 'POST' && path.includes('/approvals/')) {
      const { allow } = JSON.parse(String(init.body));
      return new Promise<Response>((settle) => answers.push({ allow, settle }));
    }
    const asked = { id: 'm', turn: 't', time: '', role: 'user', kind: 'text', body: 'Made-up ask' };
    const turns = [{ id: 't', status: 'running', why: null, ref: 'r', agent: 'a' }];
    const approvals = [{ turn: 't', request_id: 'q1', tool: 'Bash', input: { command: 'made-up --cmd' } }];
    return Promise.resolve(new Response(JSON.stringify({ chat: 'c', messages: [asked], turns, approvals })));
  });
  const opened: string[] = [];
  const screen = await render(
    <ChatView
      chat={{ id: 'c', title: 'Made-up chat' }}
      agents={[]}
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
      onReference={(ref) => opened.push(ref)}
    />,
  );
  const card = screen.getByRole('group', { name: 'Allow Bash?' });
  await card.getByRole('button', { name: 'Deny', exact: true }).click();
  // Sent, not yet taken: the card stays, and can't be answered twice.
  await expect.element(card.getByText('Denying…')).toBeVisible();
  await expect.element(card.getByRole('button', { name: 'Deny', exact: true })).not.toBeInTheDocument();

  const refusal = { error: 'Answer failed', why: 'made-up reason', ref: 'r-answer' };
  answers[0].settle(new Response(JSON.stringify(refusal), { status: 500 }));
  const notice = screen.getByRole('alert').filter({ hasText: 'Answer failed' });
  await expect.element(notice.getByText('made-up reason')).toBeVisible();
  // Focus goes back to the answer that failed, so the keyboard can answer again.
  await expect.element(card.getByRole('button', { name: 'Deny', exact: true })).toHaveFocus();
  await notice.getByRole('button', { name: 'Reference r-answer' }).click();
  expect(opened).toEqual(['r-answer']);

  await card.getByRole('button', { name: 'Allow', exact: true }).click();
  await expect.element(notice).not.toBeInTheDocument();
  answers[1].settle(new Response('{}'));
  await expect.element(card).not.toBeInTheDocument();
  expect(answers.map((a) => a.allow)).toEqual([false, true]);
  vi.unstubAllGlobals();
});
