import type { PersonTask, PlanSummary, PlanVersionSummary, Task } from '../api';
import { inWords } from '../chat/statusWords';
import type { Mark } from '../components/marks';

/**
 * A task's state on its line (a row, a task's view), in the chat's words; the same for a main message in the chat
 * (0075), so the chat, the Tasks list and the status line never disagree.
 */
export type Said =
  /** It waits on the person, in the attention colour: "Waiting" on a row, "Needs you" in the chat. */
  | { you: true; word?: string }
  /** A state the marks draw, in the mark's shape, still; waiting says on what when it is one thing. */
  | { mark: Extract<Mark, 'working' | 'stopped' | 'blocked' | 'waiting'>; on?: string }
  /** An ending with the chat's icon; failed in the danger colour. */
  | { icon: 'merge' | 'failed' | 'cancelled' }
  | { word: string }
  /** Done: a tick; a row says no word, the chat says Done (or Answered, Approved). */
  | { done: true; word?: string };

const ENDED = new Set<Task['state']>(['done', 'cancelled', 'failed', 'stopped']);

/** Whether it is over: done, cancelled, failed or stopped. */
export const ended = (t: Task) => ENDED.has(t.state);

/** A question that waits on the person now. */
export const forYou = (t: Task) => t.kind === 'person' && t.state === 'ready';

/** Held only until its PR merges. */
const merging = (t: Task) =>
  t.state === 'held' && !!t.waiting_on?.length && t.waiting_on.every((w) => w.endsWith(' to merge'));

export function said(t: Task, tasks: Task[]): Said {
  if (t.state === 'done') return { done: true };
  if (t.kind === 'person') {
    if (t.state === 'ready') return { you: true };
    if (t.state === 'held') return { word: 'Discussing' };
  }
  switch (t.state) {
    case 'working':
    case 'stopped':
    case 'blocked':
      return { mark: t.state };
    case 'failed':
    case 'cancelled':
      return { icon: t.state };
    case 'ready':
      return { word: 'Not started' };
    case 'held': {
      if (merging(t)) return { icon: 'merge' };
      // On what, as the status line says it, when it is one thing.
      const on = t.waiting_on?.length === 1 ? inWords(t.waiting_on[0]) : undefined;
      return on ? { mark: 'waiting', on } : { mark: 'waiting' };
    }
  }
  // Waiting: for the tasks it comes after that aren't done, else for something else.
  const before = t.after.filter((n) => tasks.find((o) => o.number === n)?.state !== 'done');
  return before.length ? { word: `After ${before.map((n) => `#${n}`).join(', ')}` } : { mark: 'waiting' };
}

/** A task's or a question's state as its main message in the chat says it (0075): `said`, with Done in words. */
export function mainSaid(t: Task, tasks: Task[]): Said {
  const s = said(t, tasks);
  if (t.kind === 'person' && 'done' in s) return { done: true, word: 'Answered' };
  if (t.kind === 'person' && 'you' in s) return { you: true, word: 'Needs you' };
  return 'done' in s ? { done: true, word: 'Done' } : s;
}

/** A version of the lead's plan as its main message says it: the newest by its plan's state, an older by how it ended. */
export function versionSaid(plan: PlanSummary, version: PlanVersionSummary): Said | undefined {
  if (version.decided === 'approved') return { done: true, word: 'Approved' };
  if (version.decided === 'sent_back') return { word: 'Sent back' };
  if (version.decided === 'dropped') return { word: 'Dropped' };
  if (plan.versions.at(-1)?.id !== version.id) return undefined;
  switch (plan.state) {
    case 'checking':
      return { word: 'Checking' };
    case 'waiting':
      return { you: true, word: 'Needs you' };
    case 'revising':
      return { mark: 'working' };
    default:
      return undefined;
  }
}

/** The person's message as its main message says it: Working while anything it started is open, else Done. */
export const openSaid = (open: boolean): Said => (open ? { mark: 'working' } : { done: true, word: 'Done' });

/** The chat's tasks as the Tasks tab lists them: questions for the person, what is open, what is over. */
export function grouped(tasks: Task[]): { title: string; tasks: Task[] }[] {
  const up = [...tasks].sort((a, b) => a.number - b.number);
  return [
    { title: 'For you', tasks: up.filter(forYou) },
    { title: 'Open', tasks: up.filter((t) => !forYou(t) && !ended(t)) },
    { title: 'Closed', tasks: up.filter(ended).reverse() },
  ].filter((g) => g.tasks.length > 0);
}

/** A question as its block reads it, from the chat's tasks: the member it was asked for is its request's giver. */
export function asQuestion(t: Task, tasks: Task[]): PersonTask {
  return {
    ...t,
    chat_title: '',
    for_agent: tasks.find((r) => r.number === t.request)?.giver ?? null,
    why: t.why ?? null,
    form: t.form ?? 'text',
    choices: t.choices ?? [],
    files: t.files ?? [],
  };
}
