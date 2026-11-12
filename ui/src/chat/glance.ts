import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Member, Pauses } from '../api';
import type { Custom } from './messages';
import { agentPause, type Run } from './pauses';
import { type ChatState, unanswered } from './state';
import { clock, list } from './team';
import { began, open, waitingOn } from './waits';

/** Where an agent stands, most pressing first: it needs the person, works, waits or is held, or is ready. */
export type Standing = 'needs' | 'working' | 'waiting' | 'ready';

/** One agent of the chat at a glance (DESIGN.md "Summary"), from the service's records only. */
export interface Glance {
  member: Member;
  standing: Standing;
  /** What it is doing, in one phrase: its row in Summary. */
  doing: string;
  /** Working: when its longest-running turn began, as a clock time. */
  began?: string;
  /** Waiting: the same phrase with when each wait began. Start times show in the agent's panel only. */
  detail?: string;
  /** Its tasks in this chat, cancelled ones left out. */
  done: number;
  total: number;
}

const ORDER: Standing[] = ['needs', 'working', 'waiting', 'ready'];

/**
 * Every agent of the chat: who needs the person first, then who works, who waits, who is ready; the
 * Supervisor first among equals. `activity` is what each running turn's row says its agent is doing now.
 */
export function glances(state: ChatState, pauses: Pauses, run: Run, activity: Map<string, string>): Glance[] {
  const all = state.members.map((member): Glance => {
    const tasks = state.tasks.filter((t) => t.owner === member.agent && t.state !== 'cancelled');
    return {
      member,
      ...standing(member, state, pauses, run, activity),
      done: tasks.filter((t) => t.state === 'done').length,
      total: tasks.length,
    };
  });
  const lead = (g: Glance) => (g.member.role === 'supervisor' ? 0 : 1);
  return all.sort((a, b) => ORDER.indexOf(a.standing) - ORDER.indexOf(b.standing) || lead(a) - lead(b));
}

function standing(
  m: Member,
  state: ChatState,
  pauses: Pauses,
  run: Run,
  activity: Map<string, string>,
): Pick<Glance, 'standing' | 'doing' | 'began' | 'detail'> {
  const turns = Object.values(state.turns).filter((t) => t.agent === m.agent);
  if (state.approvals.some((a) => unanswered(a) && state.turns[a.turn]?.agent === m.agent))
    return { standing: 'needs', doing: 'Needs your approval' };
  if (state.tasks.some((t) => t.kind === 'person' && t.giver === m.agent && open(t)))
    return { standing: 'needs', doing: 'Asked you a question' };
  const running = turns.filter((t) => t.status === 'running');
  if (running.length) {
    const starts = running
      .map((t) => t.created ?? '')
      .filter(Boolean)
      .sort();
    return { standing: 'working', doing: activity.get(m.agent) ?? 'Working', began: clock(starts[0]) };
  }
  const own = agentPause(pauses, m.agent);
  const held = (doing: string) => ({ standing: 'waiting' as const, doing });
  if (own.usageUntil)
    return held(`Usage limit until ${clock(new Date(own.usageUntil / 1000).toISOString())}`);
  if (run.state === 'stopped') return held('Stopped');
  const mine = state.tasks.some((t) => t.owner === m.agent && open(t));
  const queued = turns.some((t) => t.status === 'queued');
  if (own.paused || run.state !== 'running') return held(mine || queued ? 'Held' : 'Paused');
  // Each name the agent waits for, with when its earliest wait on it began.
  const name = (agent: string | null) =>
    agent === null ? 'you' : (state.members.find((x) => x.agent === agent)?.name ?? 'an agent');
  const { agents, other } = waitingOn(m.agent, state);
  const since = [...[...agents].map(([who, us]) => [name(who), us] as const), ...other];
  if (since.length)
    return {
      ...held(`Waiting for ${list(since.map(([who]) => who))}`),
      detail: `Waiting for ${list(since.map(([who, us]) => `${who} since ${began(us)}`))}`,
    };
  return mine || queued ? held('Waiting') : { standing: 'ready', doing: 'Ready' };
}

const names = (all: Glance[], standing: Standing) =>
  all.filter((g) => g.standing === standing).map((g) => g.member.name);

/** Who needs the person and who is working, in words: "Vale needs you, Juno and Pia working". */
export function headline(all: Glance[]): string {
  const needs = names(all, 'needs');
  const working = names(all, 'working');
  return [
    needs.length && `${list(needs)} ${needs.length === 1 ? 'needs' : 'need'} you`,
    working.length && `${list(working)} working`,
  ]
    .filter(Boolean)
    .join(', ');
}

/** The same as counts, beside Summary's Team heading: "2 working · 1 needs you". */
export function counts(all: Glance[]): string {
  const needs = names(all, 'needs').length;
  const working = names(all, 'working').length;
  return [working && `${working} working`, needs && `${needs} ${needs === 1 ? 'needs' : 'need'} you`]
    .filter(Boolean)
    .join(' · ');
}

/** What each agent with a running turn is doing now, as that turn's row in the timeline labels it. */
export function activity(items: ThreadMessageLike[]): Map<string, string> {
  const doing = new Map<string, string>();
  for (const item of items) {
    const { by, work } = (item.metadata?.custom ?? {}) as Custom;
    if (by && work?.running) doing.set(by, work.label);
  }
  return doing;
}
