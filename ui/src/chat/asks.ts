import type { EventLine } from './messages';
import type { ChatState } from './state';
import type { Team } from './team';

// The service's own word (asks.rs `FIXED`): how its line for a repair whose work is in begins.
const FIXED = 'Fixed: ';

/** Gathr's line for a repair whose work is in: the ask's first line, opening to its tasks' lines. */
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

/** Gathr's line for a held turn's caught change (0063): its first line, opening to the files. */
export function changed(text: string): EventLine {
  const [first, ...files] = text.split('\n');
  return { kind: 'changed', text: first, more: files.join('\n') || undefined };
}

/** An ask's size as its own message shows it (0061): who sized it, an agent by name, or the person. */
export interface AskSize {
  ask: string;
  size: 'repair' | 'new';
  by?: { agent: string; name: string };
  yours?: boolean;
}

/**
 * Each ask's size, by its message's id (the ask's own): it sits on the person's message that started the ask, so it
 * shows as soon as there is one, whoever has spoken. An unsized ask with no task, that Gathr never held, shows none
 * (0061 G5).
 */
export function askSizes(state: ChatState, crew: Team): Map<string, AskSize> {
  const tasked = new Set(state.tasks.map((t) => t.ask));
  const sizes = new Map<string, AskSize>();
  for (const ask of state.asks) {
    if (!ask.size && !ask.held && !tasked.has(ask.id)) continue;
    const size: AskSize = { ask: ask.id, size: ask.size === 'repair' ? 'repair' : 'new' };
    if (ask.size && ask.by === 'person') size.yours = true;
    else if (ask.size && ask.by) size.by = { agent: ask.by, name: crew.name(ask.by) ?? 'An agent' };
    sizes.set(ask.id, size);
  }
  return sizes;
}
