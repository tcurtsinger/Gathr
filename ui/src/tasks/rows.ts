import type { PersonTask, Task } from '../api';
import type { Mark } from '../components/marks';

/** A task's state on its line (a row, a task's view), in the chat's words. */
export type Said =
  /** A question waits on the person: "Waiting", in the attention colour. */
  | { you: true }
  /** A state the marks draw, in the mark's shape, still. */
  | { mark: Extract<Mark, 'working' | 'stopped' | 'blocked' | 'waiting'> }
  /** An ending with the chat's icon; failed in the danger colour. */
  | { icon: 'merge' | 'failed' | 'cancelled' }
  | { word: string }
  /** Done: no word, a tick after its what. */
  | { done: true };

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
    case 'held':
      return merging(t) ? { icon: 'merge' } : { mark: 'waiting' };
  }
  // Waiting: for the tasks it comes after that aren't done, else for something else.
  const before = t.after.filter((n) => tasks.find((o) => o.number === n)?.state !== 'done');
  return before.length ? { word: `After ${before.map((n) => `#${n}`).join(', ')}` } : { mark: 'waiting' };
}

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
