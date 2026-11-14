import type { Task } from '../api';

/** One task a hear-back reports, as that hear-back said it: its state then, not as the task stands now. */
export interface Reported {
  task: Task;
  state: string;
  reason?: string;
}

// The service's own words (tasks/words.rs `hear_back`): a first line, then "- #<number> <owner>, <what>: <how>"
// for each task, where <how> is "done", "<state>" or "<state>: <reason>". A task that stopped as its
// owner left it, or with no report, says "left: <what is left>" or "no report: <why>" (0047).
const FIRST =
  /^(?:The (?:task|(\d+) tasks) you gave (?:is|are) settled|Where the (?:task|(\d+) tasks) you gave stands?):$/;
// What Gathr says in the chat, in no turn, when a task ends done (tasks/words.rs `DONE`, 0051): this
// line, then the task's.
const DONE = 'A task is done:';
const TASK = /^- #(\d+) /;

/** The task whose end one of Gathr's lines says: said once there, so a hear-back needn't say it again. */
export function ended(text: string, tasks: Task[]): number | undefined {
  return text.startsWith(`${DONE}\n`) ? hearBack(text, tasks)?.[0]?.task.number : undefined;
}

/**
 * The tasks a hear-back reports, or the one a line for a finished task names, read from its own lines.
 * None for anything else Gathr says (a wait's result, a wake-up), or for a hear-back that can't be read
 * whole: that message is drawn in its own words.
 *
 * A reason is written as the agent gave it, so it can run over lines and hold a line that looks like a
 * task's. The first line says how many tasks there are; once a reason is open, a line is another task's
 * only if tasks are still due and it names one of the chat's tasks with its own words.
 */
export function hearBack(text: string, tasks: Task[]): Reported[] | undefined {
  const [first, ...lines] = text.split('\n');
  const head = FIRST.exec(first);
  if (!head && first !== DONE) return undefined;
  const due = Number(head?.[1] ?? head?.[2] ?? 1);
  // A hear-back closes with Gathr's instruction to its agent, never part of a reason; a line for a
  // finished task has none, so a line stands in for it.
  const rest = head ? lines : [...lines, ''];
  const reported: Reported[] = [];
  for (const [at, line] of rest.entries()) {
    const last = reported.at(-1);
    const next = reported.length < due ? taskLine(line, tasks) : undefined;
    if (next) reported.push(next);
    else if (last?.reason !== undefined && at < rest.length - 1) last.reason += `\n${line}`;
    else if (TASK.test(line)) return undefined;
  }
  if (reported.length !== due) return undefined;
  return reported.map((r) => ({ ...r, reason: r.reason?.trim() || undefined }));
}

function taskLine(line: string, tasks: Task[]): Reported | undefined {
  const number = TASK.exec(line)?.[1];
  const task = number ? tasks.find((t) => t.number === Number(number)) : undefined;
  // The owner's name may have changed since, or hold a comma; the task's words are as they were.
  const cut = task ? line.indexOf(`, ${task.what}: `) : -1;
  if (!task || cut < 0) return undefined;
  const [state, ...reason] = line.slice(cut + task.what.length + 4).split(': ');
  return { task, state, reason: reason.length ? reason.join(': ') : undefined };
}

const OUTCOME: Record<string, string> = {
  done: 'completed',
  failed: 'failed',
  stopped: 'stopped',
  cancelled: 'cancelled',
  blocked: 'is blocked',
  left: 'left it unfinished',
  'no report': 'gave no report',
  // Done, and held only until its merge lands.
  held: 'finished, waiting to merge',
};

/** What became of a task, after its owner's name: "Cass completed", "Cass is blocked". */
export const outcome = (state: string) => OUTCOME[state] ?? 'is working on';
