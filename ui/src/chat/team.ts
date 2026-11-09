import type { Member, Role, Task, Turn } from '../api';
import type { ChatState } from './state';

/**
 * Where a team's work sits (DESIGN.md "Team messages", 0015): a turn on a task belongs in the thread
 * under the main-line turn that gave it, followed up through tasks its own turns gave. Only the
 * service's links decide it (each turn's `task`, each task's `given_in`), never prose or timing.
 */
export interface Team {
  /** The main-line turn whose thread a turn's reply sits in; none for the main line. */
  rootOf: (turn: string) => string | undefined;
  /** Each thread's root, with its tasks in the order they were given. */
  threads: Map<string, Task[]>;
  name: (agent: string | null | undefined) => string | undefined;
  /** The agent's role, shown only in a chat with more than one agent. */
  role: (agent: string | null | undefined) => Role | undefined;
}

export function team(state: ChatState, names: Map<string, string>): Team {
  const tasks = new Map(state.tasks.map((t) => [t.number, t]));
  const givenIn = (turn: string) => {
    // A turn on a task answers the turn that gave it; the task says so too, once loaded.
    const t = state.turns[turn];
    if (t?.task == null) return undefined;
    return tasks.get(t.task)?.given_in ?? t.answers ?? undefined;
  };
  const roots = new Map<string, string | undefined>();
  const rootOf = (turn: string): string | undefined => {
    if (roots.has(turn)) return roots.get(turn);
    roots.set(turn, undefined); // a loop of links ends here
    const above = givenIn(turn);
    // A turn on no task that answers one inside a thread (a member hearing back, or woken) stays
    // in that thread; one that answers a main-line turn is the lead's own word, on the main line.
    const answers = state.turns[turn]?.answers;
    const root =
      above && state.turns[above]
        ? (rootOf(above) ?? above)
        : answers && state.turns[turn]?.task == null
          ? rootOf(answers)
          : undefined;
    roots.set(turn, root);
    return root;
  };
  const threads = new Map<string, Task[]>();
  for (const t of [...state.tasks].sort((a, b) => a.number - b.number)) {
    if (!t.given_in || !state.turns[t.given_in]) continue;
    const root = rootOf(t.given_in) ?? t.given_in;
    threads.set(root, [...(threads.get(root) ?? []), t]);
  }
  const roles = new Map(state.members.map((m: Member) => [m.agent, m.role]));
  return {
    rootOf,
    threads,
    name: (agent) =>
      agent ? (names.get(agent) ?? state.members.find((m) => m.agent === agent)?.name) : undefined,
    role: (agent) => (agent && state.members.length > 1 ? roles.get(agent) : undefined),
  };
}

export const roleName: Record<Role, string> = {
  supervisor: 'Supervisor',
  reviewer: 'Reviewer',
  member: 'Member',
};

const order: Task['state'][] = [
  'working',
  'waiting',
  'ready',
  'held',
  'done',
  'blocked',
  'failed',
  'stopped',
  'cancelled',
];

/**
 * The thread's disclosure in a line: who is working, who is waiting, who is done, and who replied after
 * the lead's last word. "Kit and Lane working · Rowan done · Sol waiting".
 */
export function describe(
  tasks: Task[],
  late: { agent: string; time: string }[],
  name: (agent: string) => string | undefined,
): string {
  const states = new Map<string, Task['state']>();
  for (const t of tasks) {
    const was = states.get(t.owner);
    if (was === undefined || order.indexOf(t.state) < order.indexOf(was)) states.set(t.owner, t.state);
  }
  const parts: string[] = [];
  const lateOnes = new Set(late.map((l) => l.agent));
  for (const l of late) parts.push(`${name(l.agent) ?? 'An agent'} replied at ${clock(l.time)}`);
  for (const state of order) {
    const who = [...states]
      .filter(([a, s]) => s === state && !lateOnes.has(a))
      .map(([a]) => name(a) ?? 'an agent');
    if (who.length) parts.push(`${list(who)} ${word(state)}`);
  }
  return parts.join(' · ');
}

function word(state: Task['state']): string {
  return state === 'ready' || state === 'held' ? 'waiting' : state;
}

export function list(names: string[]): string {
  return names.length < 2 ? (names[0] ?? '') : `${names.slice(0, -1).join(', ')} and ${names.at(-1)}`;
}

/** A message's time as the reader's clock shows it: 09:42. */
export function clock(time: string | undefined): string {
  const at = time ? new Date(time) : null;
  if (!at || Number.isNaN(at.getTime())) return '';
  return at.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
}

/** The thread is open while its round runs and folds once the lead's last word on it is in. */
export function finished(final: Turn | undefined): boolean {
  return !!final && final.status !== 'running' && final.status !== 'queued';
}
