import type { Message, ToolBody, Turn } from '../api';
import type { ChatState } from './state';

/** What the transcript draws, in order: tool runs fold into one row, and a turn's
 * streaming text and its notice sit where its reply belongs. */
export type Row =
  | { kind: 'message'; message: Message }
  | { kind: 'tools'; id: string; tools: ToolBody[] }
  | { kind: 'streaming'; turn: string; text: string }
  | { kind: 'notice'; turn: Turn; replied: boolean };

const noticed = new Set(['stopped', 'waiting_sign_in', 'failed']);

export function rows(state: ChatState): Row[] {
  const last = new Map<string, number>();
  state.messages.forEach((m, i) => {
    if (m.turn) last.set(m.turn, i);
  });
  const out: Row[] = [];
  const replied = new Set<string>();
  state.messages.forEach((m, i) => {
    if (m.kind === 'tool') {
      const prev = out.at(-1);
      const body = m.body as ToolBody;
      if (prev?.kind === 'tools') prev.tools.push(body);
      else out.push({ kind: 'tools', id: m.id, tools: [body] });
    } else {
      out.push({ kind: 'message', message: m });
    }
    if (m.role === 'assistant' && m.turn) replied.add(m.turn);
    if (m.turn && last.get(m.turn) === i) endOfTurn(state, m.turn, replied.has(m.turn), out);
  });
  return out;
}

function endOfTurn(state: ChatState, id: string, replied: boolean, out: Row[]) {
  const text = state.partial[id];
  if (text) out.push({ kind: 'streaming', turn: id, text });
  const turn = state.turns[id];
  if (turn && noticed.has(turn.status)) out.push({ kind: 'notice', turn, replied: replied || !!text });
}
