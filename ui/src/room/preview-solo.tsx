// SCRATCH for design mock-ups only; not committed.
import { StrictMode, useState } from 'react';
import { createRoot } from 'react-dom/client';
import type { Member, Message, ToolBody, Turn } from '../api';
import { Composer } from '../components/Composer';
import { Face, type FaceOf, KnownFaces } from '../components/Face';
import type { Picked } from '../components/mention';
import { WorkspaceToggle } from '../components/workspace';
import { TopLine } from '../TopLine';
import { people, Room } from './Room';
import type { RoomState } from './useRoom';
import '../theme.css';
import '../app.css';

const fresh = location.hash.includes('fresh');
const agents: FaceOf[] = [{ id: 'mira', name: 'Mira', shape: 'cloud', eyes: 'glance', colour: 'orange' }];
const members: Member[] = [{ agent: 'mira', name: 'Mira', role: 'member' }];
const start = Date.now() - 12 * 60_000;
const at = (m: number) => new Date(start + m * 60_000).toISOString();
const turns: Turn[] = [];
const messages: Message[] = [];
let n = 0;
function user(min: number, body: string) {
  messages.push({ id: `u${++n}`, turn: null, time: at(min), role: 'user', kind: 'text', body });
}
function turn(from: number, took: number | null, words: string[], tools: [string, string][]) {
  const id = `t${turns.length + 1}`;
  turns.push({
    id,
    agent: 'mira',
    status: took === null ? 'running' : 'done',
    why: null,
    ref: '',
    created: at(from),
    ended: took === null ? null : at(from + took),
  });
  const of = { turn: id, time: at(from), role: 'assistant' as const, reply_to: null };
  for (const [tool, text] of tools) {
    const body: ToolBody = { tool, status: took === null ? 'running' : 'ok', input: { text } };
    messages.push({ id: `m${++n}`, ...of, kind: 'tool', body });
  }
  for (const body of words) messages.push({ id: `m${++n}`, ...of, kind: 'text', body });
  return id;
}
if (!fresh) {
  user(0, 'What changed in the lease renewal they sent? It is in Downloads.');
  turn(
    0.1,
    0.9,
    [
      [
        'Three things changed from last year:',
        '',
        '| Term | Last year | Renewal |',
        '|---|---|---|',
        '| Rent | $2,150 | $2,290 |',
        '| Length | 12 months | 18 months |',
        '| Pets | Allowed | $40 a month |',
        '',
        'The longer term is the one to look at: breaking it early costs two months of rent (section 14).',
      ].join('\n'),
    ],
    [
      ['Read', '~/Downloads/lease-2027.pdf'],
      ['Read', '~/Documents/lease-2026.pdf'],
    ],
  );
  user(3, 'Write a short reply asking for 12 months at the new rent.');
  turn(
    3.1,
    0.5,
    [
      [
        'Here is a draft:',
        '',
        "> Hi Dana, thanks for sending the renewal. We would like to stay, and the new rent works for us. Could we keep the term at 12 months, as in this year's lease? Happy to sign once that is changed.",
        '',
        'Want it shorter, or should I mention the pet fee too?',
      ].join('\n'),
    ],
    [],
  );
  user(6, 'Also check if the pet fee is allowed in this state.');
  turn(
    6.1,
    null,
    [],
    [
      ['WebSearch', 'pet rent limits residential lease'],
      ['WebFetch', 'state tenant handbook'],
    ],
  );
}
const tid = turns.at(-1)?.id;
const room: RoomState = {
  messages,
  turns: Object.fromEntries(turns.map((t) => [t.id, t])),
  partial: tid && !fresh ? { [tid]: 'Reading the state tenant handbook for pet fees.' } : {},
  under: {},
  members: fresh ? [] : members,
  stays: [],
  reactions: [],
  approvals: [],
  questions: [],
  background: [],
};
function Preview() {
  const [draft, setDraft] = useState<{ text: string; picks: Picked[] }>({ text: '', picks: [] });
  const [sidebar, setSidebar] = useState(false);
  const everyone = people(members, agents);
  const team = members.map((m) => ({ id: m.agent, name: m.name }));
  const acts = { approve: () => undefined, ask: () => undefined, like: () => undefined };
  return (
    <div className="app">
      <div className="rail" />
      <nav
        className="sidebar"
        style={{ background: 'var(--frame)', borderRight: '1px solid var(--border)' }}
      />
      <TopLine>
        <div className="room-title">
          {!fresh && <Face agent={agents[0]} size={20} />}
          <h1>{fresh ? 'New chat' : 'Mira'}</h1>
        </div>
        {!fresh && (
          <div className="room-actions">
            <WorkspaceToggle open={sidebar} onToggle={() => setSidebar(!sidebar)} />
          </div>
        )}
      </TopLine>
      <main className="chat room" aria-label="Mira">
        <Room
          chat="preview"
          state={room}
          people={everyone}
          acts={acts}
          solo
          header={
            fresh && (
              <div className="room-greeting">
                <Face agent={agents[0]} size={56} />
                <p>Good evening, Travis</p>
              </div>
            )
          }
        >
          <Composer
            draft={draft.text}
            picks={draft.picks}
            onDraft={(text, picks) => setDraft({ text, picks })}
            onSend={() => setDraft({ text: '', picks: [] })}
            onStop={fresh ? undefined : () => undefined}
            hint="Message Mira"
            people={team}
            to={team}
          />
        </Room>
      </main>
    </div>
  );
}
if (location.hash.includes('light')) document.documentElement.classList.add('light');
createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <KnownFaces.Provider value={new Map(agents.map((a) => [a.id, a]))}>
      <Preview />
    </KnownFaces.Provider>
  </StrictMode>,
);
