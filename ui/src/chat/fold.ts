import type { Message, Task, ThinkingBody, ToolBody } from '../api';

/** One thing a turn did before its message: a tool call, a thought, or words it wrote mid-work. */
export type Step =
  | { id: string; say: string }
  | {
      id: string;
      label: string;
      /** What it says while it runs ("Running"); a thought has none. */
      doing?: string;
      /** Its chip: what it ran ("git merge origin/main"), or a task it gave ("#119 Open the settings tools PR"). */
      detail?: string;
      /** The task it gave, which its chip opens. */
      task?: number;
      state: ToolBody['status'];
      /** Gathr refused it: a call to one of Gathr's own tools that answered an error. */
      refused?: boolean;
      /** What it ran, whole (0044): a command's full text, else its chip. */
      request?: string;
      /** The first lines of what came back (0044), "…" after them when more did. */
      result?: string;
    };

/** A version of the lead's plan, as its chip in the bubble opens it (0062). */
export interface Chip {
  plan: number;
  version: number;
  parts: number;
}

/**
 * A piece of a turn's block in its bubble, in the order it came (0082): words of its message, by their place among the
 * message's text parts, or the chip of a plan version it wrote, where it wrote it (0078). Its steps and narration are
 * the one line at the bubble's foot.
 */
export type Piece = { id: string; words: number } | { id: string; plan: Chip };

/** What a turn did, as one row: a run of its steps in the chat, all of them in the agent's panel (0037). */
export interface Work {
  steps: Step[];
  running: boolean;
  /** Finished: "Worked for 1m 49s", or for a run what it did ("Read 2 files, ran 1 command"). Running: what the agent is doing now. */
  label: string;
  /** After the label: "6 steps" (what it ran, never its thoughts), "gave 2 tasks". */
  notes: string[];
  /** When it began (ms), for a running row's elapsed time. */
  since?: number;
  /** Finished: how long it took, "1m 49s". */
  took?: string;
  /** How many of its steps failed. */
  failed?: number;
  /** How many of its calls to Gathr's own tools Gathr refused. */
  refused?: number;
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
  mcp__gathr__post: ['Chose where to post', 'Choosing where to post'],
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
  mcp__gathr__react: ['Gave a thumbs-up', 'Giving a thumbs-up'],
};

/** Gathr's calls that only report or place: words followed by nothing else are still the message (0082). */
const NOT_A_STEP = new Set(['mcp__gathr__react', 'mcp__gathr__finish_task', 'mcp__gathr__post']);

// The label beside a chip that says what ran, which needs no "a command" or "a file" after it.
const BESIDE: Record<string, [string, string]> = {
  Bash: ['Ran', 'Running'],
  Shell: ['Ran', 'Running'],
  PowerShell: ['Ran', 'Running'],
  Read: ['Read', 'Reading'],
  Edit: ['Edited', 'Editing'],
  Write: ['Edited', 'Editing'],
};

// What a finished run's line counts, by kind ("Read 2 files, ran 1 command"); any other tool is a plain step.
const COUNTED: Record<string, [string, string]> = {
  Bash: ['ran', 'command'],
  Shell: ['ran', 'command'],
  PowerShell: ['ran', 'command'],
  Read: ['read', 'file'],
  Edit: ['edited', 'file'],
  Write: ['edited', 'file'],
};
// A give_task call counts as a task given only when Gathr made the task.
const GAVE: [string, string] = ['gave', 'task'];

/**
 * A length of time: to the second as a row shows it ("9s", "1m 49s", "1h 5m"; nothing under a second), or
 * in whole minutes for a line redrawn once a minute ("1m" at least, "12m", "1h 5m").
 */
export function duration(ms: number, to: 'second' | 'minute' = 'second'): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  // A wait can last days (0050): "6d 19h" reads where "163h 53m" doesn't.
  if (minutes >= 2880) return `${Math.floor(minutes / 1440)}d ${Math.floor((minutes % 1440) / 60)}h`;
  if (minutes >= 60) return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
  if (to === 'minute') return `${Math.max(1, minutes)}m`;
  if (!(seconds >= 1)) return '';
  return minutes ? `${minutes}m ${String(seconds % 60).padStart(2, '0')}s` : `${seconds}s`;
}

const THINKING = 'Thinking';
const GATHRS = 'mcp__gathr__';

/**
 * A thought, as its step (0042): "Thinking" while it runs, "Thought for 9s" after, with nothing to open.
 * One that took under a second is left out.
 */
function thought(id: string, { status, duration_ms }: ThinkingBody): Step | [] {
  if (status === 'running') return { id, label: THINKING, state: 'running' };
  const took = duration(duration_ms ?? 0);
  return took ? { id, label: `Thought for ${took}`, state: status === 'done' ? 'ok' : 'stopped' } : [];
}

/**
 * Thoughts with nothing between them, as one: an engine thinks in pieces (Codex's come back to back), and
 * a person reads them as one thought. Its time is theirs together; its state is the last one's, or running
 * while any of them is.
 */
function joined(msgs: Message[]): Message[] {
  const out: Message[] = [];
  for (const m of msgs) {
    const held = out.at(-1);
    if (m.kind !== 'thinking' || held?.kind !== 'thinking') {
      out.push(m);
      continue;
    }
    const [soFar, piece] = [held.body as ThinkingBody, m.body as ThinkingBody];
    const duration_ms = (soFar.duration_ms ?? 0) + (piece.duration_ms ?? 0);
    const status = soFar.status === 'running' ? soFar.status : piece.status;
    out[out.length - 1] = { ...held, body: { ...piece, status, duration_ms } };
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
 * A block of a turn, folded (0082). `pieces` are what its bubble draws: its message's words (`words`, text still
 * streaming last) and its plan versions' chips; `line` is the one line at the bubble's foot, every step and every piece
 * of narration in order, the working line while the turn runs. `work` is the agent panel's row: everything before the
 * block's last text, mid-work words as steps; a block that only wrote once has none. `gave` holds the task each
 * give_task call made; `began` is when the turn did, if the block is its first; `chips` the plan versions it wrote.
 */
export function fold(
  msgs: Message[],
  streaming: string,
  running: boolean,
  gave: Map<string, Given>,
  began?: string,
  chips?: Map<string, Chip>,
): { work?: Work; words: string[]; pieces: Piece[]; line?: Work } {
  const times = [began, ...msgs.map((m) => m.time)]
    .map((t) => Date.parse(t ?? ''))
    .filter((t) => !Number.isNaN(t));
  const since = times.length ? Math.min(...times) : undefined;
  const laid = piecesOf(msgs, streaming, running, gave, since, chips);
  // A version's message is its chip, never words or a step.
  const said = chips?.size ? msgs.filter((m) => !chips.has(m.id)) : msgs;
  const last = said.at(-1);
  const text = streaming || (last?.kind === 'text' ? (last.body as string) : undefined);
  const before = streaming || !text ? said : said.slice(0, -1);
  const all = joined(before);
  const steps = all.flatMap((m) => stepOf(m, gave));
  if (steps.length === 0) return laid;
  const calls = all.filter((m) => m.kind === 'tool');
  const given = calls.filter((m) => gave.has(m.id)).length;
  const bad = steps.filter((s) => 'state' in s && s.state === 'error');
  const refused = bad.filter((s) => 'refused' in s && s.refused).length;
  const failed = bad.length - refused;
  const notes = [
    calls.length && plural(calls.length, 'step'),
    given && `gave ${plural(given, 'task')}`,
  ].filter((n): n is string => !!n);
  if (running) {
    const label = doingOf(all) ?? 'Working';
    return { ...laid, work: { steps, running, label, notes, since, failed, refused } };
  }
  const ends = msgs
    .map((m) => Date.parse(m.time) + ((m.kind !== 'text' && (m.body as ToolBody).duration_ms) || 0))
    .filter((t) => !Number.isNaN(t));
  const took = since === undefined ? '' : duration(Math.max(...ends) - since);
  const label = took ? `Worked for ${took}` : 'Worked';
  return { ...laid, work: { steps, running, label, notes, ...(took && { took }), failed, refused } };
}

/** A step the message rule counts (0082): one the line draws, but no call of Gathr's that only reports or places. */
const isStep = (m: Message, gave: Map<string, Given>) =>
  m.kind !== 'text' &&
  !(m.kind === 'tool' && NOT_A_STEP.has((m.body as ToolBody).tool)) &&
  !Array.isArray(stepOf(m, gave));

/**
 * What a block's bubble draws (0082, the owner's "Inside the steps"): its message, the words it wrote after its last
 * step (else, once it has ended, its last words), with its plan chips where they came; every other piece of words is
 * narration, drawn in the line at the foot in order with the steps. While the turn runs, the words being written show
 * as words until a step follows them; a running turn with nothing yet has the working line alone.
 */
function piecesOf(
  msgs: Message[],
  streaming: string,
  running: boolean,
  gave: Map<string, Given>,
  since?: number,
  chips?: Map<string, Chip>,
): { words: string[]; pieces: Piece[]; line?: Work } {
  const all = joined(msgs);
  const said = (m: Message) => m.kind === 'text' && !!m.body && !chips?.has(m.id);
  const after = all.findLastIndex((m) => isStep(m, gave)) + 1;
  const message = new Set(all.slice(after).filter(said));
  if (!message.size && !streaming && !running) {
    const last = all.findLast(said);
    if (last) message.add(last);
  }
  const words: string[] = [];
  const pieces: Piece[] = [];
  const steps: Step[] = [];
  for (const m of all) {
    const chip = chips?.get(m.id);
    if (chip) pieces.push({ id: m.id, plan: chip });
    else if (message.has(m)) {
      pieces.push({ id: m.id, words: words.length });
      words.push(m.body as string);
    } else if (m.kind !== 'text' || m.body) steps.push(...[stepOf(m, gave)].flat());
  }
  if (streaming) {
    pieces.push({ id: 'streaming', words: words.length });
    words.push(streaming);
  }
  const tools = all.filter((m) => m.kind !== 'text');
  if (steps.length) return { words, pieces, line: lineOf(tools, steps, running, gave, since) };
  const working = { steps, running, label: 'Working', notes: [], since };
  return { words, pieces, ...(running && { line: working }) };
}

/** A turn's steps as its line: while it runs, the working line; finished, what it did, counted by kind. */
function lineOf(
  msgs: Message[],
  steps: Step[],
  running: boolean,
  gave: Map<string, Given>,
  since?: number,
): Work {
  const label = running ? (doingOf(msgs) ?? 'Working') : tally(msgs, steps, gave);
  return { steps, running, label, notes: [], since };
}

/**
 * What a finished run did: "Read 2 files, ran 1 command", at most three kinds, the rest "and 2 more"; with no tool of a
 * counted kind, "2 steps". A thought isn't counted: a run of only a thought reads as it does ("Thought for 9s").
 */
function tally(msgs: Message[], steps: Step[], gave: Map<string, Given>): string {
  const kinds = new Map<string, number>();
  let other = 0;
  for (const m of msgs) {
    // A call that only reports or places (a thumbs-up, finishing its task) isn't what the turn did.
    if (m.kind !== 'tool' || NOT_A_STEP.has((m.body as ToolBody).tool)) continue;
    const kind = gave.has(m.id) ? GAVE : COUNTED[(m.body as ToolBody).tool];
    if (!kind) other += 1;
    else kinds.set(kind.join(' '), (kinds.get(kind.join(' ')) ?? 0) + 1);
  }
  const counted = [...kinds];
  if (!counted.length)
    return other ? plural(other, 'step') : (steps.findLast((s) => 'label' in s)?.label ?? 'Worked');
  const more = other + counted.slice(3).reduce((sum, [, n]) => sum + n, 0);
  const said = counted
    .slice(0, 3)
    .map(([kind, n]) => {
      const [verb, noun] = kind.split(' ');
      return `${verb} ${plural(n, noun)}`;
    })
    .join(', ');
  return `${said[0].toUpperCase()}${said.slice(1)}${more ? ` and ${more} more` : ''}`;
}

/** What the agent is doing now: its latest running step, as its row says it ("Running a command", "Thinking"). */
function doingOf(msgs: Message[]): string | undefined {
  const now = msgs.findLast(
    (m) => m.kind !== 'text' && (m.body as ToolBody | ThinkingBody).status === 'running',
  );
  if (!now) return undefined;
  if (now.kind === 'thinking') return THINKING;
  const { tool } = now.body as ToolBody;
  return TOOLS[tool]?.[1] ?? tool;
}

/** One message as a step of the panel's row: a tool call, a thought, or words written mid-work. */
function stepOf(m: Message, gave: Map<string, Given>): Step | [] {
  if (m.kind === 'text') return { id: m.id, say: m.body as string };
  if (m.kind === 'thinking') return thought(m.id, m.body as ThinkingBody);
  const body = m.body as ToolBody;
  const [did, now] = TOOLS[body.tool] ?? [body.tool, body.tool];
  const made = gave.get(m.id);
  // What it ran (0041): the engine's own words for it when it gave some, else a short label, then the chip.
  // A message from before steps kept their input has none, and reads as it did.
  const ran = made ? undefined : body.input;
  const [didIt, doingIt] = (ran && BESIDE[body.tool]) || [did, now];
  // Gathr's own tools answer an error only to refuse: the step says so, never what it would have done.
  const refused = body.status === 'error' && body.tool.startsWith(GATHRS);
  const result = body.result && (body.result.more ? `${body.result.text}\n…` : body.result.text);
  return {
    id: m.id,
    label: refused ? 'Refused' : made?.owner ? `Gave ${made.owner} a task` : (ran?.title ?? didIt),
    doing: ran?.title ?? doingIt,
    detail: made
      ? `#${made.task.number} ${made.task.look_into ? 'Look into: ' : ''}${made.task.what}`
      : ran?.text,
    task: made?.task.number,
    state: body.status,
    ...(refused && { refused }),
    request: ran?.request ?? ran?.text,
    result,
  };
}
