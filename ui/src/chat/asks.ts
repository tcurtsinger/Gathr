import type { Ask } from '../api';
import type { EventLine, Group } from './messages';
import type { ChatState } from './state';
import type { Team } from './team';

// The service's own word (asks.rs `FIXED`): how its line for a repair whose work is in begins.
const FIXED = 'Fixed: ';

/** Gathr's line for a repair whose work is in: the ask's title, opening to its tasks' lines. */
export function fixed(text: string): EventLine | undefined {
  if (!text.startsWith(FIXED)) return undefined;
  const [first, ...tasks] = text.split('\n');
  return {
    kind: 'task',
    who: 'Fixed:',
    text: first.slice(FIXED.length),
    more: tasks.join('\n') || undefined,
  };
}

/**
 * Each ask's size line, by the block it follows (0061 G5.2): the first block below the message of the
 * lead's turn that sized it, else of the message's own turn, so a line never sits above the message it
 * sizes. The message can land in a later turn than the one that sized it, when the engine reads it only
 * after that turn ends. An unsized ask with no task, that Gathr never held, has none. `groups` is in the
 * thread's order.
 */
export function sizeLines(state: ChatState, groups: Group[], crew: Team): Map<Group, EventLine[]> {
  const lines = new Map<Group, EventLine[]>();
  for (const ask of state.asks) {
    if (!ask.size && !ask.held && !state.tasks.some((t) => t.ask === ask.id)) continue;
    const from = groups.findIndex((g) => g.msgs.some((m) => m.id === ask.id));
    const own = groups[from]?.turn;
    if (!own) continue;
    const below = (turn: string | null) =>
      groups.slice(from + 1).find((g) => g.kind === 'reply' && g.turn === turn);
    const after = (ask.sized_in && below(ask.sized_in)) || below(own);
    if (after) lines.set(after, [...(lines.get(after) ?? []), line(ask, crew)]);
  }
  return lines;
}

/** "Mira: Fix", "You: Plan", or "Plan" alone while unsized. */
function line(ask: Ask, crew: Team): EventLine {
  const kind = ask.size === 'repair' ? 'fix' : 'plan';
  const text = kind === 'fix' ? 'Fix' : 'Plan';
  if (!ask.size) return { kind, text, ask: ask.id };
  const who = ask.by === 'person' ? 'You:' : `${crew.name(ask.by) ?? 'An agent'}:`;
  return { kind, who, text, ask: ask.id };
}
