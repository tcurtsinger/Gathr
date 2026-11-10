import type { Member, Role } from '../api';
import type { ChatState } from './state';

/**
 * Where a team's work sits (DESIGN.md "Team messages", 0015): a turn on a task belongs to the hand-off
 * of the main-line turn that gave it, followed up through tasks its own turns gave, and shows in the
 * team column. Only the service's links decide it (each turn's `task`, each task's `given_in`), never
 * prose or timing.
 */
export interface Team {
  /** The main-line turn whose hand-off a turn works on; none for the main line. */
  rootOf: (turn: string) => string | undefined;
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
  const roles = new Map(state.members.map((m: Member) => [m.agent, m.role]));
  return {
    rootOf,
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

export function list(names: string[]): string {
  return names.length < 2 ? (names[0] ?? '') : `${names.slice(0, -1).join(', ')} and ${names.at(-1)}`;
}

/** A message's time as the reader's clock shows it: 09:42. */
export function clock(time: string | undefined): string {
  const at = time ? new Date(time) : null;
  if (!at || Number.isNaN(at.getTime())) return '';
  return at.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
}
