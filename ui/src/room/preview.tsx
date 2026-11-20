import { StrictMode, useState } from 'react';
import { createRoot } from 'react-dom/client';
import type { BoardLine, Member, Message, ToolBody, Turn } from '../api';
import { Board } from '../Board';
import { Composer } from '../components/Composer';
import type { FaceOf } from '../components/Face';
import type { Picked } from '../components/mention';
import { TopLine } from '../TopLine';
import { people, Room } from './Room';
import type { RoomState } from './useRoom';
import '../theme.css';
import '../app.css';

// A made-up chat, as in the mock-up: Mira leads, Ann and Bo build, Rev reviews. Times count back from now. Bo's turn
// waits for an approval, and the person liked Ann's message.
const agents: FaceOf[] = [
  { id: 'mira', name: 'Mira', face_seed: 'Mira', background: '#619eff' },
  { id: 'ann', name: 'Ann', face_seed: 'Ann', background: '#ffd166' },
  { id: 'bo', name: 'Bo', face_seed: 'Bo', background: '#9be3b4' },
  { id: 'rev', name: 'Rev', face_seed: 'Rev', background: '#f4a6c6' },
];
const members: Member[] = [
  { agent: 'mira', name: 'Mira', role: 'supervisor' },
  { agent: 'ann', name: 'Ann', role: 'member' },
  { agent: 'bo', name: 'Bo', role: 'member' },
  { agent: 'rev', name: 'Rev', role: 'reviewer' },
];

const start = Date.now() - 25 * 60_000;
const at = (min: number) => new Date(start + min * 60_000).toISOString();
const turns: Turn[] = [];
const messages: Message[] = [];
let n = 0;

function turn(
  agent: string,
  from: number,
  took: number | null,
  /** The main message it replies to, as the service sets `reply_to`; '' for a main message. */
  reply: string,
  words: string[],
  tools: string[],
) {
  const id = `t${turns.length + 1}`;
  const status = took === null ? 'running' : 'done';
  const ended = took === null ? null : at(from + took);
  turns.push({ id, agent, status, why: null, ref: '', created: at(from), ended });
  const of = { turn: id, time: at(from), role: 'assistant' as const, reply_to: reply || null };
  for (const tool of tools) {
    const body: ToolBody = { tool, status: took === null ? 'running' : 'ok', input: { text: input(tool) } };
    messages.push({ id: `m${++n}`, ...of, kind: 'tool', body });
  }
  for (const body of words) messages.push({ id: `m${++n}`, ...of, kind: 'text', body });
  return id;
}
/** A turn's last words: what a reply names, and what a thumbs-up goes on. */
const words = (id: string) => messages.findLast((m) => m.turn === id && m.kind === 'text')?.id ?? '';
const input = (tool: string) =>
  ({ Read: 'src/orders/page.tsx', Bash: 'npm test -- orders', Edit: 'src/orders/export.rs' })[tool] ??
  'orders';

messages.push({
  id: 'u1',
  turn: null,
  time: at(0),
  role: 'user',
  kind: 'text',
  body: 'Add a CSV export to the orders page.',
});
const plan = turn(
  'mira',
  0.2,
  0.8,
  '',
  [
    [
      'Three pieces, on the board now:',
      '',
      '- @Ann the export itself, and a column picker after it.',
      '- @Bo the button on the orders page.',
      "- I'll add the help line once it works.",
      '',
      "I'd build the file in the page, since it's the least code.",
    ].join('\n'),
  ],
  ['Read', 'Read', 'Read', 'Read'],
);
turn(
  'rev',
  2,
  0.6,
  words(plan),
  [
    'That will freeze the page. The orders table has 40,000 rows, and I timed the same loop on the invoices page at 9 seconds. Build it in the service and send it as a download.',
  ],
  ['Read', 'Bash'],
);
turn('mira', 3, 0.3, words(plan), ['Fair. The service builds it. Board unchanged.'], []);
const pr = turn(
  'ann',
  5.8,
  6.2,
  '',
  [
    'The export is up as `#291`: `GET /orders/export` sends the file in pieces, so memory stays flat. @Rev can you look?',
  ],
  [...Array(6).fill('Read'), ...Array(4).fill('Edit'), ...Array(4).fill('Bash')],
);
turn(
  'rev',
  12,
  1,
  words(pr),
  [
    "One defect. Dates are written in the server's time zone: an order placed at 23:30 lands on the next day in the file. I ran it with one such order. Nothing else.",
  ],
  ['Bash'],
);
turn(
  'ann',
  17,
  1,
  words(pr),
  ["Fixed in `2c1f`: dates go out in the shop's time zone. Your 23:30 order now lands on its own day."],
  ['Edit', 'Bash'],
);
turn('ann', 19, 0.2, words(pr), ['Merged #291.'], ['Bash']);
const bo = turn('bo', 23, null, '', [], ['Read', 'Read', 'Edit', 'Edit', 'Bash']);

const room: RoomState = {
  messages,
  turns: Object.fromEntries(turns.map((t) => [t.id, t])),
  partial: { [bo]: 'Wiring the button to the new download, then checking it with 40,000 rows.' },
  under: {},
  members,
  reactions: [
    { message: words(plan), agent: 'ann', time: at(1) },
    { message: words(plan), agent: 'bo', time: at(1) },
    { message: words(pr), agent: 'rev', time: at(13) },
    { message: words(pr), agent: 'person', time: at(14) },
  ],
  approvals: [
    { turn: bo, request: 'r1', tool: 'Bash', input: { command: 'npm run e2e -- orders --rows 40000' } },
  ],
};
const board: BoardLine[] = [
  { id: 1, owner: 'ann', name: 'Ann', text: 'Export from the service', state: 'done' },
  { id: 2, owner: 'bo', name: 'Bo', text: 'Export button', state: 'doing' },
  { id: 3, owner: 'ann', name: 'Ann', text: 'Column picker', state: 'to_do' },
  { id: 4, owner: 'mira', name: 'Mira', text: 'Help line for the export', state: 'to_do' },
];
// The board asks the service for its lines: here they are made up too.
const fetched = window.fetch;
window.fetch = (url, init) =>
  String(url).endsWith('/board')
    ? Promise.resolve(new Response(JSON.stringify({ lines: board, agents: 4 })))
    : fetched(url, init);

const ignore = () => {};

function Preview() {
  const [draft, setDraft] = useState<{ text: string; picks: Picked[] }>({ text: '', picks: [] });
  const [state, setState] = useState(room);
  const team = members.map((m) => ({ id: m.agent, name: m.name }));
  const acts = {
    approve: (request: string) =>
      setState((s) => ({ ...s, approvals: s.approvals.filter((a) => a.request !== request) })),
    reference: ignore,
    like: (message: string, give: boolean) =>
      setState((s) => ({
        ...s,
        reactions: give
          ? [...s.reactions, { message, agent: 'person', time: new Date().toISOString() }]
          : s.reactions.filter((r) => r.message !== message || r.agent !== 'person'),
      })),
  };
  return (
    <div className="app">
      <div className="rail" />
      <nav
        className="sidebar"
        style={{ background: 'var(--frame)', borderRight: '1px solid var(--border)' }}
      />
      <TopLine>
        <div className="room-title">
          <h1>Orders export</h1>
        </div>
      </TopLine>
      <main className="chat room" aria-label="Orders export">
        <Room state={state} people={people(members, agents)} acts={acts}>
          <Composer
            draft={draft.text}
            picks={draft.picks}
            onDraft={(text, picks) => setDraft({ text, picks })}
            onSend={() => setDraft({ text: '', picks: [] })}
            hint="Message the team"
            people={team}
            to={draft.picks.length ? draft.picks : team.slice(0, 1)}
          />
        </Room>
      </main>
      <Board chat="preview" onFailure={ignore} />
    </div>
  );
}

if (location.hash === '#light') document.documentElement.classList.add('light');
createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <Preview />
  </StrictMode>,
);
