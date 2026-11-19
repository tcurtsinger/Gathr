import type { Turn } from '../api';

/**
 * What the person can do about how a block's turn ended (0051), under its bubble: Continue, Retry or Sign in, or
 * `reference`, its reference alone with nothing to press (a failed turn a like opened, 0057). How it ended is no word on
 * the name line (the owner's call of 2026-11-18).
 */
export interface Ending {
  turn: Turn;
  act: 'continue' | 'retry' | 'sign-in' | 'reference';
}

/**
 * The ending of a turn that failed, waits for a sign-in, or was stopped by the person or a restart; none for one that
 * runs, finished, waits for its engine's download, or that Gathr ended for a reason of its own (nothing to carry on).
 * `carried`: its carry-on has started, so nothing is left to continue.
 */
export function ending(turn: Turn, carried: boolean): Ending | undefined {
  if (turn.status === 'failed') return { turn, act: 'retry' };
  if (turn.status === 'waiting_sign_in') return { turn, act: 'sign-in' };
  if (turn.status !== 'stopped' || carried) return undefined;
  const end = turn.end ?? 'you';
  return end === 'you' || end === 'restart' ? { turn, act: 'continue' } : undefined;
}
