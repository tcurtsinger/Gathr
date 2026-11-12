import type { Task } from '../api';

/** One task a hear-back reports, as that hear-back said it: its state then, not as the task stands now. */
export interface Reported {
  task: Task;
  state: string;
  reason?: string;
}

// The service's own words (tasks/words.rs `hear_back`): a first line, then "- #<number> <owner>, <what>: <how>"
// for each task, where <how> is "done", "<state>" or "<state>: <reason>".
const FIRST =
  /^(?:The (?:task|\d+ tasks) you gave (?:is|are) settled|Where the (?:task|\d+ tasks) you gave stands?):$/;
const TASK = /^- #(\d+) /;

/**
 * The tasks a hear-back reports, read from its own lines. None for anything else Gathr says (a wait's
 * result, a wake-up), or for a hear-back that can't be read whole: that message is drawn in its own words.
 */
export function hearBack(text: string, tasks: Task[]): Reported[] | undefined {
  const [first, ...rest] = text.split('\n');
  if (!FIRST.test(first)) return undefined;
  const reported: Reported[] = [];
  for (const [at, line] of rest.entries()) {
    const number = TASK.exec(line)?.[1];
    if (!number) {
      // A reason of several lines carries on; the last line is Gathr's closing instruction.
      const last = reported.at(-1);
      if (last?.reason && at < rest.length - 1) last.reason += `\n${line}`;
      continue;
    }
    const task = tasks.find((t) => t.number === Number(number));
    // The owner's name may have changed since; the task's words haven't.
    const cut = task ? line.indexOf(`, ${task.what}: `) : -1;
    if (!task || cut < 0) return undefined;
    const [state, ...reason] = line.slice(cut + task.what.length + 4).split(': ');
    reported.push({ task, state, reason: reason.join(': ') || undefined });
  }
  return reported.length ? reported : undefined;
}

const OUTCOME: Record<string, string> = {
  done: 'completed',
  failed: 'failed',
  stopped: 'stopped',
  cancelled: 'cancelled',
  blocked: 'is blocked',
  // Done, and held only until its merge lands.
  held: 'finished, waiting to merge',
};

/** What became of a task, after its owner's name: "Cass completed", "Cass is blocked". */
export const outcome = (state: string) => OUTCOME[state] ?? 'is working on';
