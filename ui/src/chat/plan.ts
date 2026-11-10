import type { AgentPlanStatus } from '@/components/assistant-ui/elements/agent-plan';
import type { Task, ToolBody } from '../api';
import type { ChatState } from './state';
import { list, team } from './team';
import { open } from './waits';

export interface Step {
  task: number;
  label: string;
  owner: string;
  state: AgentPlanStatus;
  /** A working step's latest tool call; a held, blocked, failed or stopped step's stored reason. */
  detail?: string;
}

/** The Supervisor's current round (DESIGN.md "Agent plan"): the tasks given from its newest hand-off. */
export interface Round {
  /** The main-line turn whose hand-off it is. */
  root: string;
  steps: Step[];
  done: number;
  /** Every task settled and the Supervisor's hear-back on them ended. */
  finished: boolean;
  /** "Kit and Lane working", "Lane working, 1 blocked", "Done": the bar's line beside its title. */
  status: string;
  /** Someone in the chat has a turn running. */
  busy: boolean;
}

const outcomes = ['blocked', 'failed', 'stopped'] as const;

export function stepState(t: Task): AgentPlanStatus {
  if (t.state === 'done') return 'done';
  if (t.state === 'working' || t.state === 'held') return 'active';
  if ((outcomes as readonly string[]).includes(t.state)) return t.state as AgentPlanStatus;
  return 'pending';
}

/**
 * The round's steps are tasks only, from the service's links (each task's `given_in`, followed up
 * through tasks given inside the hand-off to its main-line turn), never prose. The current round is the
 * newest such turn of the Supervisor's with tasks. Questions for the person and cancelled tasks are left out.
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

  const steps = [...tasks]
    .sort((a, b) => a.number - b.number)
    .map(
      (t): Step => ({
        task: t.number,
        label: t.what,
        owner: crew.name(t.owner) ?? '',
        state: stepState(t),
        detail: t.state === 'working' ? latestTool(state, t) : (t.reason ?? undefined),
      }),
    );
  const done = steps.filter((s) => s.state === 'done').length;
  const hearBacks = Object.values(state.turns).filter((u) => u.answers === root && u.agent === lead);
  const finished =
    tasks.every((t) => !open(t)) &&
    hearBacks.length > 0 &&
    hearBacks.every((u) => !['queued', 'waiting_sign_in', 'running'].includes(u.status));
  const running = Object.values(state.turns).filter((u) => u.status === 'running');
  const working = state.members.filter((m) => running.some((u) => u.agent === m.agent)).map((m) => m.name);
  const counts = outcomes
    .map((o) => [o, steps.filter((s) => s.state === o).length] as const)
    .filter(([, n]) => n > 0)
    .map(([o, n]) => `${n} ${o}`);
  const status = [finished ? 'Done' : working.length ? `${list(working)} working` : undefined, ...counts]
    .filter(Boolean)
    .join(', ');
  return { root, steps, done, finished, status, busy: working.length > 0 };
}

/** As the tool's own row titles it: "Running tool: Bash", from the owner's running turn on the task. */
function latestTool(state: ChatState, t: Task): string | undefined {
  const turns = new Set(
    Object.values(state.turns)
      .filter((u) => u.task === t.number && u.agent === t.owner && u.status === 'running')
      .map((u) => u.id),
  );
  const tool = state.messages.findLast((m) => m.kind === 'tool' && m.turn !== null && turns.has(m.turn));
  if (!tool) return undefined;
  const body = tool.body as ToolBody;
  const verb = { running: 'Running', ok: 'Used', error: 'Failed', stopped: 'Cancelled' }[body.status];
  return `${verb} tool: ${body.tool}`;
}
