import type { Own } from '../api';
import { type Mark, pressing } from '../components/marks';
import { list } from './team';

/** An agent as the status line speaks of it: its state and, waiting, the one thing it waits on. */
export interface Said {
  name: string;
  state: Mark;
  on?: string;
}

/** One part of the line, coloured by its state. */
export interface Part {
  text: string;
  tone: Mark;
}

// The marks' order (0071), most pressing first. Free is never said.
const ASKS = ['needs', 'cant'] as const;
const MOVES = ['working', 'stopped', 'blocked', 'waiting'] as const;
const VERB = { needs: ['needs you', 'need you'], cant: ['can’t run', 'can’t run'] };

/**
 * The status line's words (the status-line design's rule): who needs you or can't run is named, up to two; the
 * rest name one or two agents in all and count from three, "agents" and the verb said once. What a wait is on is
 * said only when the line waits alone and every wait is on one thing.
 */
export function parts(team: Said[]): Part[] {
  const of = (state: Mark) => team.filter((a) => a.state === state);
  const names = (agents: Said[]) => list(agents.map((a) => a.name));
  const out: Part[] = [];
  for (const state of ASKS) {
    const l = of(state);
    if (!l.length) continue;
    const who = l.length <= 2 ? names(l) : `${l.length} agents`;
    out.push({ text: `${who} ${VERB[state][l.length === 1 ? 0 : 1]}`, tone: state });
  }
  const moving = MOVES.map((state) => [state, of(state)] as const).filter(([, l]) => l.length);
  const n = moving.reduce((sum, [, l]) => sum + l.length, 0);
  moving.forEach(([state, l], i) => {
    const text =
      n <= 2
        ? `${names(l)} ${l.length === 1 ? 'is' : 'are'} ${state}`
        : i === 0
          ? `${l.length} ${l.length === 1 ? 'agent is' : 'agents are'} ${state}`
          : `${l.length} ${state}`;
    out.push({ text, tone: state });
  });
  const waits = of('waiting');
  const on = waits[0]?.on;
  if (out.length === 1 && out[0].tone === 'waiting' && on && waits.every((a) => a.on === on)) {
    out[0].text += ` on ${on}`;
  }
  return out;
}

/** The dots move only while an agent, or Gathr, works. */
export const moving = (team: Said[]) => team.some((a) => a.state === 'working');

/**
 * Gathr as the line speaks of it, when it has anything of its own going: said like an agent, in the most pressing
 * of its states, and waiting on the one thing it waits on, when it is one.
 */
export function gathrSaid(own: Own[]): Said | undefined {
  const state = pressing(own.map((o) => o.state));
  if (!state) return undefined;
  const waits = [...new Set(own.filter((o) => o.state === 'waiting').map((o) => inWords(o.what)))];
  return { name: 'Gathr', state, on: waits.length === 1 ? waits[0] : undefined };
}

/**
 * A pull request's wait in the approved pages' words: "PR #18 checks", "PR #18 to merge", "PR #18 to land". The
 * service names them "CI on #18", "#18 to merge or close" (`tasks::wait_name`) and "#18 to land", as agents read
 * them through the `waiting` tool; anything else is said as the service says it.
 */
export function inWords(what: string): string {
  return what
    .replace(/^CI on #(\d+)$/, 'PR #$1 checks')
    .replace(/^#(\d+) to merge or close$/, 'PR #$1 to merge')
    .replace(/^#(\d+) to land$/, 'PR #$1 to land')
    .replace(/^#(\d+) to be up to date$/, 'PR #$1 to be up to date');
}
