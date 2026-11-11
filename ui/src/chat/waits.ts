import type { Task, Wait } from '../api';
import type { ChatState } from './state';
import { clock } from './team';

export const open = (t: Task) => !['done', 'blocked', 'failed', 'stopped', 'cancelled'].includes(t.state);

/**
 * What an agent waits on, each with when its earliest wait on it began (µs), from the service's records:
 * agents (null for the person) by their open waits, the tasks it gave and its own tasks' blockers; and
 * its other open waits (CI, a PR, a check) by name.
 */
export function waitingOn(agent: string, state: ChatState) {
  const agents = new Map<string | null, number>();
  const other = new Map<string, number>();
  const on = <K>(map: Map<K, number>, key: K, us: number) => map.set(key, Math.min(us, map.get(key) ?? us));
  for (const w of state.waits.filter((w) => w.agent === agent && w.state === 'open')) {
    if (w.kind === 'agent') on(agents, w.target, w.created_ts);
    else on(other, waitName(w), w.created_ts);
  }
  const mine = state.tasks.filter((t) => t.owner === agent && open(t));
  for (const t of mine) {
    for (const b of state.tasks.filter((b) => t.after.includes(b.number) && open(b)))
      on(agents, b.owner, t.created_us);
  }
  for (const t of state.tasks.filter((t) => t.giver === agent && open(t))) on(agents, t.owner, t.created_us);
  return { agents, other };
}

/** When a wait began: its time today, else its date (Nov 8). */
export function began(us: number): string {
  const at = new Date(us / 1000);
  if (at.toDateString() === new Date().toDateString()) return clock(at.toISOString());
  return at.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

/** A wait in a few words, as the service names it (tasks/words.rs `wait_name`). */
function waitName(w: Wait): string {
  const target = w.target ?? '';
  if (w.kind === 'ci') return /^\d+$/.test(target) ? `CI on #${target}` : `CI on ${target.slice(0, 7)}`;
  if (w.kind === 'pr') return `#${target} to merge or close`;
  if (w.kind === 'up_to_date') return `#${target} to be up to date`;
  return 'a check';
}
