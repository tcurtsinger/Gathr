// A reply streaming into a long team chat draws only itself again (0072): the bubbles, name lines and threads above it
// stay as they were drawn. React's own commit hook says which components ran (renderWatch.ts).
import './renderWatch';
import { useRef } from 'react';
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Agent, Message, Turn } from '../api';
import { toThread } from './messages';
import { watchRenders } from './renderWatch';
import { type ChatState, empty } from './state';
import { type Actions, Thread } from './Thread';

const members = [
  { agent: 'a', name: 'Made-up Lead', role: 'supervisor' as const },
  { agent: 'b', name: 'Made-up Member', role: 'member' as const },
];
const agents: Agent[] = members.map((m) => ({
  id: m.agent,
  name: m.name,
  account: 'x',
  model: null,
  effort: null,
  approvals: 'auto',
  face_seed: 'Made-up',
  gender: null,
  skin: null,
  background: null,
}));
const none = new Proxy({}, { get: () => () => {} }) as Actions;

function View({ state }: { state: ChatState }) {
  const cache = useRef(new Map());
  return (
    <div style={{ height: 900, display: 'flex' }}>
      <Thread
        messages={toThread(state, agents, [], cache.current).items}
        running
        actions={none}
        empty={false}
      />
    </div>
  );
}

/** Twenty rounds: the person asks, the lead answers after a step, and the member writes to the lead, who answers. */
function chat(): ChatState {
  const turns: Record<string, Turn> = {};
  const messages: Message[] = [];
  const say = (id: string, turn: string, body: string, more: Partial<Message> = {}) =>
    messages.push({ id, turn, time: '2026-11-08T09:00:00Z', role: 'assistant', kind: 'text', body, ...more });
  for (let n = 0; n < 20; n++) {
    turns[`l${n}`] = { id: `l${n}`, agent: 'a', status: 'done', why: null, ref: 'r', position: 3 * n };
    turns[`m${n}`] = { id: `m${n}`, agent: 'b', status: 'done', why: null, ref: 'r', position: 3 * n + 1 };
    turns[`t${n}`] = {
      id: `t${n}`,
      agent: 'a',
      status: 'done',
      why: null,
      ref: 'r',
      position: 3 * n + 2,
      answers: `m${n}`,
    };
    say(`u${n}`, `l${n}`, `Made-up ask ${n}`, { role: 'user' });
    say(`s${n}`, `l${n}`, '', { kind: 'tool', body: { tool: 'Read', status: 'ok' } });
    say(`a${n}`, `l${n}`, `Made-up **answer** ${n} with \`code\`.`);
    say(`w${n}`, `m${n}`, `Made-up work ${n}`);
    say(`q${n}`, `m${n}`, `Made-up question ${n}`, { to: 'a' });
    say(`c${n}`, `t${n}`, 'Made-up words, carried', { role: 'gathr', copy_of: `q${n}` });
    say(`r${n}`, `t${n}`, `Made-up reply ${n}`, { to: 'b' });
  }
  turns.live = { id: 'live', agent: 'a', status: 'running', why: null, ref: 'r', position: 60 };
  say('ul', 'live', 'Made-up last ask', { role: 'user' });
  return { ...empty, members, turns, messages, reactions: [{ message: 'a3', agent: 'b', time: '' }] };
}

test('a reply streaming into a long team chat draws only itself again', async () => {
  const state = chat();
  const words = 'made-up streamed words '.repeat(40).split(' ');
  const screen = await render(<View state={{ ...state, partial: { live: words[0] } }} />);
  await expect.element(screen.getByText('Made-up reply 19')).toBeInTheDocument();
  const stop = watchRenders();
  for (let n = 2; n < 40; n += 2) {
    await screen.rerender(<View state={{ ...state, partial: { live: words.slice(0, n).join(' ') } }} />);
  }
  await expect.element(screen.getByText(words.slice(0, 38).join(' '))).toBeInTheDocument();
  const drawn = stop();
  // The streaming block ran on each token; nothing else did.
  expect(drawn['a-live']).toBeGreaterThan(0);
  expect(Object.keys(drawn)).toEqual(['a-live']);
});
