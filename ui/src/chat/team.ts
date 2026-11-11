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
  // Turns the person's answers woke (each answered question's `heard_in`): the Supervisor answering
  // the person, on the main line whatever task it runs on (G158). What it hands on from there is team work.
  const heard = new Set(state.tasks.filter((t) => t.kind === 'person' && t.heard_in).map((t) => t.heard_in));
  const roots = new Map<string, string | undefined>();
  const rootOf = (turn: string): string | undefined => {
    if (roots.has(turn)) return roots.get(turn);
    roots.set(turn, undefined); // a loop of links ends here
    if (heard.has(turn)) return undefined;
    const above = givenIn(turn);
    // A turn on no task that answers one inside a thread (a member hearing back, or woken) stays
    // in that thread; one that answers the agent's own main-line turn is the lead's word, on the main
    // line. One that answers another agent's turn was woken by its message: team work, as a hand-off.
    const t = state.turns[turn];
    const answered = t?.answers ? state.turns[t.answers] : undefined;
    const root =
      above && state.turns[above]
        ? (rootOf(above) ?? above)
        : answered && t?.task == null
          ? (rootOf(answered.id) ?? (answered.agent !== t.agent ? answered.id : undefined))
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
