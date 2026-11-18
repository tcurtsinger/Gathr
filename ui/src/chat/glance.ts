import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Member, Waiting } from '../api';
import { FROM_SERVICE, MARKS, type Mark, pressing, WORD } from '../components/marks';
import type { Custom } from './messages';
import { type ChatState, unanswered } from './state';
import { inWords } from './statusWords';
import { clock, list, open } from './team';

/** Where an agent stands: the one state its face shows (0071), most pressing first. */
export type Standing = Mark;

/**
 * One agent of the chat at a glance (DESIGN.md "Summary"), from the service's records only. Who needs the person
 * and who works is read from the chat as it streams; the rest is the service's answer (0050, 0071), so the window
 * works none of that out itself.
 */
export interface Glance {
  member: Member;
  standing: Standing;
  /** What it is doing, in one phrase: its row in Summary. */
  doing: string;
  /** The task its running turn works on, or the one its state is about. */
  task?: number;
  /** What its card says after the state's word: "Show cook times · 4 steps". */
  about?: string;
  /** Since when it has been in this state, in ms. */
  since?: number;
  /** Waiting: the one thing it waits on, when it is one ("PR #18 checks"). */
  on?: string;
  /** Waiting: the looks at what it waits on keep failing. */
  failing?: boolean;
  /** Its tasks in this chat, cancelled ones and plan checks left out. */
  done: number;
  total: number;
}

/** What each agent with a running turn is doing now, as that turn's row says it, and how many steps it took. */
export type Activity = Map<string, { label: string; steps?: string }>;

const FIELDS = ['standing', 'doing', 'task', 'about', 'since', 'on', 'failing', 'done', 'total'] as const;

/**
 * Every agent of the chat, the most pressing first, the Supervisor first among equals. `activity` is what each
 * running turn's row says its agent is doing now, `waiting` the service's answer to what the chat waits on.
 * An agent whose glance reads as it did in `before` keeps that object, so what draws it needn't redraw.
 */
export function glances(
  state: ChatState,
  activity: Activity,
  waiting?: Waiting,
  before: Glance[] = [],
): Glance[] {
  const all = state.members.map((member): Glance => {
    // A plan's check is Gathr's step, not work the lead handed out: Round leaves it out too.
    const tasks = state.tasks.filter((t) => t.owner === member.agent && t.state !== 'cancelled' && !t.checks);
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
  const rank = (g: Glance) => MARKS.indexOf(g.standing);
  return all.sort((a, b) => rank(a) - rank(b) || lead(a) - lead(b));
}

type Stands = Pick<Glance, 'standing' | 'doing' | 'task' | 'about' | 'since' | 'on' | 'failing'>;

/**
 * The state an agent's face shows: the most pressing of what the chat as it streams says (an approval or a
 * question of its waits on the person, a turn of its runs) and what the service answered. The answer is a moment
 * behind the chat: it never calls an agent working that has no turn running here, nor free with work of its own
 * to come, and a turn running here is newer than whatever else it said, but that the agent needs the person.
 */
export function markOf(local: { needs?: boolean; working?: boolean; owed?: boolean }, service?: Mark): Mark {
  const said =
    service === 'working' && !local.working
      ? undefined
      : service === 'free' && local.owed
        ? 'waiting'
        : local.working && service !== 'needs'
          ? undefined
          : service;
  const fallback = local.owed ? 'waiting' : 'free';
  return (
    pressing([local.needs ? 'needs' : undefined, local.working ? 'working' : undefined, said]) ?? fallback
  );
}

function standing(m: Member, state: ChatState, activity: Activity, waiting?: Waiting): Stands {
  const turns = Object.values(state.turns).filter((t) => t.agent === m.agent);
  const running = turns.filter((t) => t.status === 'running');
  const task = running.find((t) => t.task != null)?.task ?? undefined;
  const asks = state.approvals.some((a) => unanswered(a) && state.turns[a.turn]?.agent === m.agent);
  // By its giver: the chat's tasks don't say which member a question was asked for (0038). A held one is
  // being discussed, off the person's list (0065).
  const questions = state.tasks.filter(
    (t) => t.kind === 'person' && t.giver === m.agent && t.state === 'ready',
  );
  const stand = waiting?.agents.find((a) => a.agent === m.agent);
  const owed =
    state.tasks.some((t) => t.owner === m.agent && open(t)) || turns.some((t) => t.status === 'queued');
  const mark = markOf(
    { needs: asks || questions.length > 0, working: running.length > 0, owed },
    stand && FROM_SERVICE[stand.state],
  );
  const since =
    stand?.since_us != null && FROM_SERVICE[stand.state] === mark ? stand.since_us / 1000 : undefined;
  const what = (n?: number | null) => state.tasks.find((t) => t.number === n)?.what;
  const about = (...parts: (string | null | undefined)[]) => parts.filter(Boolean).join(' · ') || undefined;
  if (mark === 'needs' && asks)
    return { standing: mark, doing: 'Needs your approval', task, about: 'Approval', since };
  if (mark === 'needs' && questions.length) {
    const asked = Math.min(...questions.map((q) => q.created_us)) / 1000;
    return { standing: mark, doing: 'Asked you a question', task, about: 'Question', since: since ?? asked };
  }
  if (mark === 'working') {
    const starts = running
      .map((t) => t.created ?? '')
      .filter(Boolean)
      .sort();
    const now = activity.get(m.agent);
    return {
      standing: mark,
      doing: now?.label ?? 'Working',
      task,
      about: about(what(task), now?.steps),
      since: starts[0] ? Date.parse(starts[0]) : since,
    };
  }
  if (mark === 'free') return { standing: mark, doing: 'Free' };
  if (!stand || FROM_SERVICE[stand.state] !== mark)
    return { standing: mark, doing: mark === 'waiting' ? 'Waiting' : WORD[mark] };
  const shown = { standing: mark, task: stand.task ?? undefined, since };
  if (mark === 'waiting') {
    const until = stand.until_us ? ` until ${clock(new Date(stand.until_us / 1000).toISOString())}` : '';
    const on = stand.on.map((o) => inWords(o.what));
    return {
      ...shown,
      doing: `${stand.words}${until}`,
      about: on.length ? list(on) : (stand.what ?? undefined),
      on: on.length === 1 ? on[0] : undefined,
      failing: stand.failing || undefined,
    };
  }
  return { ...shown, doing: stand.words, about: about(stand.what, what(stand.task)) };
}

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

const names = (all: Glance[], standing: Standing) =>
  all.filter((g) => g.standing === standing).map((g) => g.member.name);

/** The same as counts, beside Summary's Team heading: "2 working · 1 needs you". */
export function counts(all: Glance[]): string {
  const needs = names(all, 'needs').length;
  const working = names(all, 'working').length;
  return [working && `${working} working`, needs && `${needs} ${needs === 1 ? 'needs' : 'need'} you`]
    .filter(Boolean)
    .join(' · ');
}

/** What each agent with a running turn is doing now, as that turn's row labels it, with its steps so far. */
export function activity(items: ThreadMessageLike[]): Activity {
  const doing: Activity = new Map();
  for (const item of items) {
    const own = (item.metadata?.custom ?? {}) as Custom;
    // A thread under a PR holds whole blocks.
    for (const { by, work } of [own, ...(own.thread ?? []).flatMap((m) => m.block ?? [])])
      if (by && work?.running)
        doing.set(by, {
          label: work.label,
          steps: work.notes.find((n) => n.endsWith('step') || n.endsWith('steps')),
        });
  }
  return doing;
}
