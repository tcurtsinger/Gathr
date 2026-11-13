import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Member, Waiting } from '../api';
import type { Custom } from './messages';
import { type ChatState, unanswered } from './state';
import { began, clock, list, open } from './team';

/** Where an agent stands, most pressing first: it needs the person, works, waits or is held, or is ready. */
export type Standing = 'needs' | 'working' | 'waiting' | 'ready';

/**
 * One agent of the chat at a glance (DESIGN.md "Summary"), from the service's records only. Who needs
 * the person and who works is read from the chat as it streams; what an agent that does neither waits
 * for is the service's answer (0050), so the window works none of that out itself.
 */
export interface Glance {
  member: Member;
  standing: Standing;
  /** What it is doing, in one phrase: its row in Summary. */
  doing: string;
  /** Working: when its longest-running turn began, as a clock time. */
  began?: string;
  /** Waiting: the same phrase with when each wait began. Start times show in the agent's panel only. */
  detail?: string;
  /** The task its running turn works on. */
  task?: number;
  /** Its tasks in this chat, cancelled ones left out. */
  done: number;
  total: number;
}

const FIELDS = ['standing', 'doing', 'began', 'detail', 'task', 'done', 'total'] as const;
const ORDER: Standing[] = ['needs', 'working', 'waiting', 'ready'];

/**
 * Every agent of the chat: who needs the person first, then who works, who waits, who is ready; the
 * Supervisor first among equals. `activity` is what each running turn's row says its agent is doing now,
 * `waiting` the service's answer to what the chat waits on.
 * An agent whose glance reads as it did in `before` keeps that object, so what draws it needn't redraw.
 */
export function glances(
  state: ChatState,
  activity: Map<string, string>,
  waiting?: Waiting,
  before: Glance[] = [],
): Glance[] {
  const all = state.members.map((member): Glance => {
    const tasks = state.tasks.filter((t) => t.owner === member.agent && t.state !== 'cancelled');
    const now: Glance = {
      member,
      ...standing(member, state, activity, waiting),
      done: tasks.filter((t) => t.state === 'done').length,
      total: tasks.length,
    };
    const was = before.find((g) => g.member === member);
    return was && FIELDS.every((f) => was[f] === now[f]) ? was : now;
  });
  const lead = (g: Glance) => (g.member.role === 'supervisor' ? 0 : 1);
  return all.sort((a, b) => ORDER.indexOf(a.standing) - ORDER.indexOf(b.standing) || lead(a) - lead(b));
}

function standing(
  m: Member,
  state: ChatState,
  activity: Map<string, string>,
  waiting?: Waiting,
): Pick<Glance, 'standing' | 'doing' | 'began' | 'detail' | 'task'> {
  const turns = Object.values(state.turns).filter((t) => t.agent === m.agent);
  const running = turns.filter((t) => t.status === 'running');
  const task = running.find((t) => t.task != null)?.task ?? undefined;
  if (state.approvals.some((a) => unanswered(a) && state.turns[a.turn]?.agent === m.agent))
    return { standing: 'needs', doing: 'Needs your approval', task };
  // By its giver: the chat's tasks don't say which member a question was asked for (0038).
  if (state.tasks.some((t) => t.kind === 'person' && t.giver === m.agent && open(t)))
    return { standing: 'needs', doing: 'Asked you a question', task };
  if (running.length) {
    const starts = running
      .map((t) => t.created ?? '')
      .filter(Boolean)
      .sort();
    return { standing: 'working', doing: activity.get(m.agent) ?? 'Working', began: clock(starts[0]), task };
  }
  // Neither: what it waits for, or that it is free, is the service's answer.
  const stand = waiting?.agents.find((a) => a.agent === m.agent);
  const mine = state.tasks.some((t) => t.owner === m.agent && open(t));
  const queued = turns.some((t) => t.status === 'queued');
  // Free, unless the chat already shows a task or a turn of its own the answer is yet to count.
  if (stand?.state === 'free' && !mine && !queued) return { standing: 'ready', doing: 'Ready' };
  if (stand?.state === 'waiting') {
    const until = stand.until_us ? ` until ${clock(new Date(stand.until_us / 1000).toISOString())}` : '';
    const since = stand.on.map((on) => `${on.what} since ${began(on.since_us)}`);
    return {
      standing: 'waiting',
      doing: `${stand.words}${until}`,
      detail: since.length ? `Waiting for ${list(since)}` : undefined,
    };
  }
  // The answer hasn't come, or is a moment behind the chat: whether anything is its to do says which.
  return mine || queued ? { standing: 'waiting', doing: 'Waiting' } : { standing: 'ready', doing: 'Ready' };
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
