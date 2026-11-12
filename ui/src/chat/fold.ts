import type { Message, Task, ThinkingBody, ToolBody } from '../api';

/** One thing a turn did before its message: a tool call, a thought, or words it wrote mid-work. */
export type Step =
  | { id: string; say: string }
  | {
      id: string;
      label: string;
      /** Its chip: what it ran ("git merge origin/main"), or a task it gave ("#119 Open the settings tools PR"). */
      detail?: string;
      /** The task it gave, which its chip opens. */
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
  /** After the label: "6 steps" (what it ran, never its thoughts), "gave 2 tasks". */
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
  Grep: ['Searched', 'Searching'],
  WebFetch: ['Fetched', 'Fetching'],
  WebSearch: ['Searched the web', 'Searching the web'],
  Agent: ['Ran a sub-agent', 'Running a sub-agent'],
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

// The label beside a chip that says what ran, which needs no "a command" or "a file" after it.
const BESIDE: Record<string, [string, string]> = {
  Bash: ['Ran', 'Running'],
  Shell: ['Ran', 'Running'],
  PowerShell: ['Ran', 'Running'],
  Read: ['Read', 'Reading'],
  Edit: ['Edited', 'Editing'],
  Write: ['Edited', 'Editing'],
};

/**
 * A length of time: to the second as a row shows it ("9s", "1m 49s", "1h 5m"; nothing under a second), or
 * in whole minutes for a line redrawn once a minute ("1m" at least, "12m", "1h 5m").
 */
export function duration(ms: number, to: 'second' | 'minute' = 'second'): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  if (minutes >= 60) return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
  if (to === 'minute') return `${Math.max(1, minutes)}m`;
  if (!(seconds >= 1)) return '';
  return minutes ? `${minutes}m ${String(seconds % 60).padStart(2, '0')}s` : `${seconds}s`;
}

const THINKING = 'Thinking';

/**
 * A thought, as its step (0042): "Thinking" while it runs, "Thought for 9s" after, with nothing to open.
 * One that took under a second is left out.
 */
function thought(id: string, { status, duration_ms }: ThinkingBody): Step | [] {
  if (status === 'running') return { id, label: THINKING, state: 'running', time: '' };
  const took = duration(duration_ms ?? 0);
  return took
    ? { id, label: `Thought for ${took}`, state: status === 'done' ? 'ok' : 'stopped', time: '' }
    : [];
}

/**
 * Thoughts with nothing between them, as one: an engine thinks in pieces (Codex's come back to back), and
 * a person reads them as one thought. Its time is theirs together; its state is the last one's.
 */
function joined(msgs: Message[]): Message[] {
  const out: Message[] = [];
  for (const m of msgs) {
    const before = out.at(-1);
    if (m.kind !== 'thinking' || before?.kind !== 'thinking') {
      out.push(m);
      continue;
    }
    const [first, last] = [before.body as ThinkingBody, m.body as ThinkingBody];
    const duration_ms = (first.duration_ms ?? 0) + (last.duration_ms ?? 0);
    out[out.length - 1] = { ...before, body: { ...last, duration_ms } };
  }
  return out;
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
  const text = streaming || (last?.kind === 'text' ? (last.body as string) : undefined);
  const before = streaming || !text ? msgs : msgs.slice(0, -1);
  if (before.length === 0) return { text };

  let doing: string | undefined;
  let given = 0;
  let tools = 0;
  const steps = joined(before).flatMap((m): Step | [] => {
    if (m.kind === 'text') return { id: m.id, say: m.body as string };
    if (m.kind === 'thinking') {
      const body = m.body as ThinkingBody;
      if (body.status === 'running') doing = THINKING;
      return thought(m.id, body);
    }
    tools += 1;
    const body = m.body as ToolBody;
    const [did, now] = TOOLS[body.tool] ?? [body.tool, body.tool];
    const made = gave.get(m.id);
    if (made) given += 1;
    if (body.status === 'running') doing = now;
    // What it ran (0041): the engine's own words for it when it gave some, else a short label, then the chip.
    // A message from before steps kept their input has none, and reads as it did.
    const ran = made ? undefined : body.input;
    const [didIt, doingIt] = (ran && BESIDE[body.tool]) || [did, now];
    return {
      id: m.id,
      label: made?.owner
        ? `Gave ${made.owner} a task`
        : (ran?.title ?? (body.status === 'running' ? doingIt : didIt)),
      detail: made ? `#${made.task.number} ${made.task.what}` : ran?.text,
      task: made?.task.number,
      state: body.status,
      time: duration(body.duration_ms ?? 0),
    };
  });
  if (steps.length === 0) return { text };
  const notes = [tools && plural(tools, 'step'), given && `gave ${plural(given, 'task')}`].filter(
    (n): n is string => !!n,
  );
  const times = [began, ...msgs.map((m) => m.time)]
    .map((t) => Date.parse(t ?? ''))
    .filter((t) => !Number.isNaN(t));
  const since = times.length ? Math.min(...times) : undefined;
  if (running) return { text, work: { steps, running, label: doing ?? 'Working', notes, since } };
  const ends = msgs
    .map((m) => Date.parse(m.time) + ((m.kind !== 'text' && (m.body as ToolBody).duration_ms) || 0))
    .filter((t) => !Number.isNaN(t));
  const took = since === undefined ? '' : duration(Math.max(...ends) - since);
  return { text, work: { steps, running, label: took ? `Worked for ${took}` : 'Worked', notes } };
}
