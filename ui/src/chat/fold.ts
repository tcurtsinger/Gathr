import type { Message, Task, ToolBody } from '../api';

/** One thing a turn did before its message: a tool call, or words it wrote mid-work. */
export type Step =
  | { id: string; say: string }
  | {
      id: string;
      label: string;
      /** A task it gave: "#119 Open the settings tools PR". */
      detail?: string;
      task?: number;
      state: ToolBody['status'];
      /** How long it took, once it ended: "27s". */
      time: string;
    };

/** What a turn did before its message, as the one row above it (0037). */
export interface Work {
  steps: Step[];
  running: boolean;
  /** Finished: "Worked for 1m 49s". Running: what the agent is doing now. */
  label: string;
  /** After the label: "6 steps", "gave 2 tasks". */
  notes: string[];
  /** When it began (ms), for a running row's elapsed time. */
  since?: number;
}

// What each tool did, and is doing while it runs. One not listed shows its own name.
const TOOLS: Record<string, [string, string]> = {
  Bash: ['Ran a command', 'Running a command'],
  Shell: ['Ran a command', 'Running a command'],
  PowerShell: ['Ran a command', 'Running a command'],
  Read: ['Read a file', 'Reading a file'],
  Edit: ['Edited a file', 'Editing a file'],
  Write: ['Edited a file', 'Editing a file'],
  mcp__gathr__give_task: ['Gave a task', 'Giving a task'],
  mcp__gathr__ask_person: ['Asked you a question', 'Asking you a question'],
  mcp__gathr__message: ['Sent a message', 'Sending a message'],
  mcp__gathr__read_chat: ['Read the chat', 'Reading the chat'],
  mcp__gathr__list_tasks: ['Listed tasks', 'Listing tasks'],
  mcp__gathr__finish_task: ['Finished its task', 'Finishing its task'],
  mcp__gathr__restart_task: ['Restarted a task', 'Restarting a task'],
  mcp__gathr__cancel_task: ['Cancelled a task', 'Cancelling a task'],
  mcp__gathr__wait: ['Started a wait', 'Starting a wait'],
  mcp__gathr__end_wait: ['Ended a wait', 'Ending a wait'],
  mcp__gathr__checks: ['Read the checks', 'Reading the checks'],
  mcp__gathr__set_checks: ['Set the checks', 'Setting the checks'],
  mcp__gathr__merge: ['Merged a pull request', 'Merging a pull request'],
};

/** A length of time as a row shows it: "9s", "1m 49s", "1h 5m"; nothing under a second. */
export function duration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  if (!(seconds >= 1)) return '';
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${String(seconds % 60).padStart(2, '0')}s`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;

/** The task a give_task call made. */
export interface Given {
  task: Task;
  /** Its owner's name. */
  owner?: string;
}

/**
 * A block of a turn, folded: everything before its last text goes in the row, and that text is the
 * message. Text still streaming counts as the last text. A block that only wrote once has no row.
 * `gave` holds the task each give_task call made; `began` is when the turn did, if the block is its first.
 */
export function fold(
  msgs: Message[],
  streaming: string,
  running: boolean,
  gave: Map<string, Given>,
  began?: string,
): { work?: Work; text?: string } {
  const last = msgs.at(-1);
  const text = streaming || (last && last.kind !== 'tool' ? (last.body as string) : undefined);
  const before = streaming || !text ? msgs : msgs.slice(0, -1);
  if (before.length === 0) return { text };

  let doing: string | undefined;
  let given = 0;
  const steps = before.map((m): Step => {
    if (m.kind !== 'tool') return { id: m.id, say: m.body as string };
    const body = m.body as ToolBody;
    const [did, now] = TOOLS[body.tool] ?? [body.tool, body.tool];
    const made = gave.get(m.id);
    if (made) given += 1;
    if (body.status === 'running') doing = now;
    return {
      id: m.id,
      label: made?.owner ? `Gave ${made.owner} a task` : body.status === 'running' ? now : did,
      detail: made && `#${made.task.number} ${made.task.what}`,
      task: made?.task.number,
      state: body.status,
      time: duration(body.duration_ms ?? 0),
    };
  });
  const tools = steps.filter((s) => 'label' in s).length;
  const notes = [tools && plural(tools, 'step'), given && `gave ${plural(given, 'task')}`].filter(
    (n): n is string => !!n,
  );
  const times = [began, ...msgs.map((m) => m.time)]
    .map((t) => Date.parse(t ?? ''))
    .filter((t) => !Number.isNaN(t));
  const since = times.length ? Math.min(...times) : undefined;
  if (running) return { text, work: { steps, running, label: doing ?? 'Working', notes, since } };
  const ends = msgs
    .map((m) => Date.parse(m.time) + ((m.kind === 'tool' && (m.body as ToolBody).duration_ms) || 0))
    .filter((t) => !Number.isNaN(t));
  const took = since === undefined ? '' : duration(Math.max(...ends) - since);
  return { text, work: { steps, running, label: took ? `Worked for ${took}` : 'Worked', notes } };
}
