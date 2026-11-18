import type { Task, Turn } from '../api';

/**
 * How a block's turn ended, as its message says it (0051): a word on its name line, and what the person can do
 * under the bubble. A stop that is the face's mark now (the person's own) has no word.
 */
export interface Ending {
  turn: Turn;
  word?: string;
  /** The word in the danger colour. */
  bad?: boolean;
  act?: 'continue' | 'retry' | 'sign-in';
}

// Gathr stopped it for a reason of its own: a word for why, and nothing for the person to carry on.
const GATHRS: Record<string, string> = { plan: 'Needs plan', usage: 'Usage limit', gathr: 'Gathr ended it' };

/**
 * The ending of a turn that stopped, failed or waits for a sign-in; none for one that runs, finished or waits for its
 * engine's download (its face's mark says that). `carried`: its carry-on has started, so nothing is left to continue.
 * A task cancelled under its turn says so on the task's own message, but a plan's check is no longer needed.
 */
export function ending(turn: Turn, task: Task | undefined, carried: boolean): Ending | undefined {
  if (turn.status === 'failed') return { turn, word: 'Failed', bad: true, act: 'retry' };
  if (turn.status === 'waiting_sign_in') return { turn, act: 'sign-in' };
  if (turn.status !== 'stopped') return undefined;
  const end = turn.end ?? 'you';
  const act = carried ? undefined : 'continue';
  if (end === 'you') return act && { turn, act };
  if (end === 'restart') return { turn, word: 'Gathr restarted', act };
  if (end === 'cancelled') return task?.checks != null ? { turn, word: 'Not needed' } : undefined;
  return { turn, word: GATHRS[end] ?? GATHRS.gathr };
}
