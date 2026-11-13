import type { Task } from '../api';
import type { ChatState } from './state';
import { open, team } from './team';

/** A step's state: a task's own, with held counted as at work. Blocked, failed and stopped never count done. */
export type StepState = 'done' | 'active' | 'pending' | 'blocked' | 'failed' | 'stopped';

export interface Step {
  task: number;
  label: string;
  /** Who has it: the agent's id, and its name. */
  agent: string;
  owner: string;
  state: StepState;
}

/** The Supervisor's current round (DESIGN.md "Summary"): the tasks given from its newest hand-off. */
export interface Round {
  /** The main-line turn whose hand-off it is. */
  root: string;
  steps: Step[];
  done: number;
}

const outcomes = ['blocked', 'failed', 'stopped'] as const;

export function stepState(t: Task): StepState {
  if (t.state === 'done') return 'done';
  if (t.state === 'working' || t.state === 'held') return 'active';
  if ((outcomes as readonly string[]).includes(t.state)) return t.state as StepState;
  return 'pending';
}

/**
 * The round's steps are tasks only, from the service's links (each task's `given_in`, followed up
 * through tasks given inside the hand-off to its main-line turn), never prose. The current round is the
 * newest such turn of the Supervisor's with tasks. Questions for the person and cancelled tasks are left out.
 * None once that round is finished and the person has written since.
 */
export function currentRound(state: ChatState): Round | undefined {
  const lead = state.members.find((m) => m.role === 'supervisor')?.agent;
  const crew = team(state, new Map());
  const rounds = new Map<string, Task[]>();
  for (const t of state.tasks) {
    if (!t.owner || !t.given_in || t.kind === 'person' || t.kind === 'person_request') continue;
    if (t.state === 'cancelled') continue;
    const root = crew.rootOf(t.given_in) ?? t.given_in;
    const by = state.turns[root]?.agent;
    if (lead && by && by !== lead) continue;
    rounds.set(root, [...(rounds.get(root) ?? []), t]);
  }
  const place = (root: string) => state.turns[root]?.position ?? -1;
  const latest = (tasks: Task[]) => Math.max(...tasks.map((t) => t.number));
  const newest = [...rounds].sort(([a, x], [b, y]) => place(a) - place(b) || latest(x) - latest(y)).at(-1);
  if (!newest) return undefined;
  const [root, tasks] = newest;
  // Finished (every task settled and the Supervisor's hear-back on them ended), it stays until the person's
  // next message; after that it is no longer the plan (ux-issues 19).
  const hearBacks = Object.values(state.turns).filter((u) => u.answers === root && u.agent === lead);
  const finished =
    tasks.every((t) => !open(t)) &&
    hearBacks.length > 0 &&
    hearBacks.every((u) => !['queued', 'waiting_sign_in', 'running'].includes(u.status));
  const heard = state.messages.findLastIndex((m) => hearBacks.some((u) => u.id === m.turn));
  if (finished && heard >= 0 && state.messages.slice(heard + 1).some((m) => m.role === 'user'))
    return undefined;

  const steps = [...tasks]
    .sort((a, b) => a.number - b.number)
    .map(
      (t): Step => ({
        task: t.number,
        label: t.what,
        agent: t.owner ?? '',
        owner: crew.name(t.owner) ?? '',
        state: stepState(t),
      }),
    );
  const done = steps.filter((s) => s.state === 'done').length;
  return { root, steps, done };
}
