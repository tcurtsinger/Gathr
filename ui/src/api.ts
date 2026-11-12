// The service's API (AGENTS.md "The API"). The page is served from the same origin as /api:
// by the Vite proxy in dev, which adds the token.

export type TurnStatus = 'queued' | 'waiting_sign_in' | 'running' | 'done' | 'stopped' | 'failed';

export interface ToolBody {
  tool: string;
  status: 'running' | 'ok' | 'error' | 'stopped';
  duration_ms?: number;
  /**
   * What it ran (0041): a command's first line, a file's path, a search's words, with secrets hidden, and
   * the engine's own words for it when it gave some. Tools that keep nothing, and messages from before, have none.
   */
  input?: { text: string; title?: string };
}

/** That the agent thought (0042): how long, and the engine's count of its tokens when it gave one. Never its text. */
export interface ThinkingBody {
  status: 'running' | 'done' | 'stopped';
  duration_ms?: number;
  tokens?: number;
}

export interface Message {
  id: string;
  turn: string | null;
  time: string;
  /** `gathr`: what Gathr told an agent (a task, a wake-up, a hear-back), never the agent's own words. */
  role: 'user' | 'assistant' | 'gathr';
  /** `text`: its body is the words. `tool` and `thinking`: its body is an object. */
  kind: string;
  body: string | ToolBody | ThinkingBody;
  /** A message an agent sent one member: that member's id. It shows as its own block (0037). */
  to?: string | null;
  /** What Gathr told the member in its turn: the message it carries, already shown as the sender's. */
  copy_of?: string | null;
}

export interface Turn {
  id: string;
  status: TurnStatus;
  why: string | null;
  ref: string;
  /** The agent it was sent to; null for a turn sent to no agent. */
  agent: string | null;
  created?: string;
  /** Its place in the chat by when it started; none until it starts. */
  position?: number | null;
  /** The task it works on: its reply sits in the thread under the turn that gave the task. */
  task?: number | null;
  /** A hear-back's: the turn whose tasks it reports on, which its reply links back to. */
  answers?: string | null;
  /** Queued but held by a pause, usage, an app update or its first engine download. */
  held?: 'pause' | 'usage' | 'update' | 'claude' | 'codex' | null;
}

export type Role = 'supervisor' | 'reviewer' | 'member';

export interface Member {
  agent: string;
  name: string;
  role: Role;
  /** How full its context is in this chat (compaction plan, K1); none before its first call. */
  context?: Context | null;
}

/** The engine's last model call's prompt in tokens, and its model's window: either null until said. */
export interface Context {
  used: number | null;
  window: number | null;
}

/** The whole app, or one chat or agent. */
export type PauseScope = { scope: 'app' } | { scope: 'chat' | 'agent'; target: string };

/**
 * A pause (0010): pausing while a turn, background command or wait's check in it still runs, paused once quiet,
 * stopped after Stop now until Restart (its resume).
 */
export interface Pause {
  scope: 'app' | 'chat' | 'agent';
  /** The chat's or agent's id; empty for the app. */
  target: string;
  state: 'pausing' | 'paused' | 'stopped';
  /** The agent that paused it; null for the person. */
  by: string | null;
}

/** Every pause, and each account out of usage until its reset, with the agents on it. */
export interface Pauses {
  pauses: Pause[];
  usage: { account: string; until_us: number; agents: string[] }[];
}

export type TaskState =
  | 'waiting'
  | 'ready'
  | 'working'
  | 'held'
  | 'done'
  | 'blocked'
  | 'failed'
  | 'stopped'
  | 'cancelled';

/** A task an agent gave another, or the person (0010). */
export interface Task {
  number: number;
  chat: string;
  /** The agent who gave it; null when the person did. */
  giver: string | null;
  /** The agent who owns it; null for the person's own. */
  owner: string | null;
  /** The turn that gave it. */
  given_in: string | null;
  /** person is a question for the person; person_request, a member's ask the Supervisor checks first. */
  kind: 'work' | 'review' | 'person' | 'person_request';
  what: string;
  /** Why it was given, and its details. */
  why?: string | null;
  state: TaskState;
  reason: string | null;
  /** Tasks it waits for before it starts. */
  after: number[];
  /** Its last move, once it has gone the stale days without one (F11). */
  stale_us?: number | null;
  created_us: number;
  /** The turn its giver heard it was settled in: a person task's answer shows there. */
  heard_in?: string | null;
  /** Its PR, and that PR's head as Gathr last saw it: the commit whose checks run shows on it. */
  pr?: number | null;
  pr_head?: string | null;
}

/** An agent's stale task, on the person's list across chats. */
export interface StaleTask extends Task {
  chat_title: string;
}

/** A question for the person (F9): the form says how it's answered; the first choice is the asker's pick. */
export interface PersonTask extends Task {
  chat_title: string;
  /** The member it was asked for; null when the Supervisor asked for itself. */
  for_agent: string | null;
  why: string | null;
  form: 'choice' | 'choices' | 'yesno' | 'done' | 'text';
  choices: { label: string; detail?: string }[];
  files: { name: string; size: number }[];
}

export interface PersonAnswer {
  task: number;
  pick?: string[];
  yes?: boolean;
  done?: boolean;
  text?: string;
}

/** What an agent waits on (0010): another agent, CI, a PR or a check. */
export interface Wait {
  id: string;
  agent: string;
  task: number | null;
  kind: string;
  target: string | null;
  state: string;
  /** When it began, in microseconds, as tasks' created_us. */
  created_ts: number;
}

export interface Project {
  id: string;
  name: string;
  folder: string;
  members: Member[];
  /** Run in order on a PR's head; only the Supervisor sets them (0036, owner's call 1455). */
  checks: ProjectCheck[];
  /** Whose result gates a merge. */
  ci: 'ci' | 'gathr' | 'both';
  merge: 'fast-forward' | 'merge' | 'squash';
}

export interface ProjectCheck {
  command: string;
  /** The agent that set it, by id, or `found` when Gathr found it in the project. */
  source: string;
  set: string;
}

/** A run of a project's checks at one commit (0036 C3): its commands in order, stopping at the first failure. */
export interface CheckRun {
  run: string;
  head: string;
  state: 'running' | 'passed' | 'failed';
  why: string | null;
  /** Once it failed: the reference its events trace from. */
  ref: string | null;
  started: string;
  ended: string | null;
  steps: CheckStep[];
}

export interface CheckStep {
  command: string;
  exit: number | null;
  duration_ms: number;
  /** The end of what it printed. */
  log: string;
}

/** What a project's settings change: only what it names; the team whole. */
export type ProjectChange = Partial<Pick<Project, 'name' | 'folder' | 'ci' | 'merge'>> & {
  members?: { agent: string; role: Role }[];
};

export interface Chat {
  id: string;
  title: string;
  /** Its project's id; none for a plain chat. */
  project?: string | null;
  folder?: string | null;
  /** When the person archived it: its work ended then (F11). */
  archived?: string | null;
  /** A turn of it runs now (0040). An archived chat carries neither this nor `needs`. */
  working?: boolean;
  /** It waits on the person: an approval, a question to them, or a sign-in. */
  needs?: boolean;
}

/** A chat whose title holds the query (0035). `time` is its last activity. */
export interface ChatHit {
  chat: string;
  title: string;
  archived: string | null;
  time: string;
}

/** A saved text message holding every typed word as a word start; `marks` are UTF-16 [start, end] in `snippet`. */
export interface MessageHit {
  chat: string;
  chat_title: string;
  archived: string | null;
  message: string;
  role: Message['role'];
  agent: string | null;
  time: string;
  snippet: string;
  marks: [number, number][];
}

/** Settings > General's stale rules (F11), in whole days. */
export interface StaleDays {
  stale_days: number;
  cancel_days: number;
}

/** Settings > General: the stale rules, and the folder that holds every project's Shared and agent folders (M1). */
export interface Settings extends StaleDays {
  agent_folder: string;
  /** Messages between agents in a row before the chat pauses; null for no limit. */
  handoff_limit: number | null;
}

/** A reusable agent (0026): its account implies the engine. Null model or effort is the engine's default. */
export interface Agent {
  id: string;
  name: string;
  account: string;
  model: string | null;
  effort: string | null;
  /** What it may do without asking (0029). */
  approvals: Approvals;
}

export type Approvals = 'ask' | 'edits' | 'auto' | 'full';

/** A model as the account's engine lists it (0025), in the engine's order. */
export interface Model {
  /** Exact model currently selected by an alias or the engine default. */
  resolved?: string | null;
  id: string;
  name: string;
  /** A family alias that always runs that family's newest model (0025). */
  latest: boolean;
  description: string | null;
  efforts: string[];
}

export interface History {
  chat: string;
  messages: Message[];
  turns: Turn[];
  /** Questions the engine asked that nobody has answered yet. */
  approvals: { turn: string; request_id: string; tool: string; input?: unknown }[];
  /** The chat's agents and their roles (0028); empty for a chat nobody joined yet. */
  members: Member[];
  /** Replies still streaming, so a chat opened mid-turn shows what arrived so far. */
  streaming?: { turn: string; agent: string | null; text: string }[];
}

export interface Engine {
  found: string | null;
  why_not_found: string | null;
  /** Gathr's own copy (0025): its version, and the last check of the release feed. */
  copy: {
    version: string | null;
    state: 'off' | 'checking' | 'idle';
    checked: string | null;
    failed: { why: string; ref: string } | null;
  };
}

export interface Account {
  id: string;
  name: string;
  engine: string;
  login: string;
  expires: string | null;
  /**
   * A sign-in from Settings: waiting (with its page once there is one, `paste` when the engine takes
   * the code that page gives, `user_code` for a device code), or failed with why and a reference.
   */
  sign_in:
    | { state: 'waiting'; url?: string; paste?: boolean; user_code?: string }
    | { state: 'failed'; why: string; ref: string; device?: boolean }
    | null;
}

/** An answer that failed: "… failed", why, and the reference `gathr diag trace` takes. */
export class Failure extends Error {
  constructor(
    readonly error: string,
    readonly why: string | null,
    readonly ref: string | null,
  ) {
    super(why ? `${error}: ${why}` : error);
  }
}

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  let response: Response;
  try {
    response = await fetch(path, {
      method,
      headers: body === undefined ? {} : { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new Failure('Connection failed', 'The service did not respond. Try again.', null);
  }
  const answer = await response.json().catch(() => null);
  if (!response.ok || answer === null) {
    const why = answer?.why ?? (answer === null ? `${path}: invalid JSON (HTTP ${response.status})` : null);
    throw new Failure(answer?.error ?? 'Request failed', why, answer?.ref ?? null);
  }
  return answer as T;
}

export const api = {
  chats: () => call<Chat[]>('GET', '/api/chats'),
  archivedChats: () => call<Chat[]>('GET', '/api/chats?archived=true'),
  /** Ends the chat's work in one step; Restore (unarchive) never starts it again. */
  archive: (chat: string) => call<unknown>('POST', `/api/chats/${chat}/archive`),
  unarchive: (chat: string) => call<unknown>('POST', `/api/chats/${chat}/unarchive`),
  deleteChat: (chat: string) => call<unknown>('DELETE', `/api/chats/${chat}`),
  createChat: (project?: string) => call<Chat>('POST', '/api/chats', project ? { project } : {}),
  projects: () => call<{ projects: Project[] }>('GET', '/api/projects').then((p) => p.projects),
  addProject: (project: { name: string; folder: string; members: { agent: string; role: Role }[] }) =>
    call<Project>('POST', '/api/projects', project),
  saveProject: (id: string, change: ProjectChange) => call<Project>('PATCH', `/api/projects/${id}`, change),
  /** One commit's checks runs, newest first, however many the project has had since. */
  checkRuns: (id: string, head: string) =>
    call<{ runs: CheckRun[] }>(
      'GET',
      `/api/projects/${id}/checks/runs?head=${encodeURIComponent(head)}`,
    ).then((r) => r.runs),
  caches: (id: string) => call<{ bytes: number }>('GET', `/api/projects/${id}/caches`).then((c) => c.bytes),
  clearCaches: (id: string) => call<unknown>('DELETE', `/api/projects/${id}/caches`),
  history: (chat: string) => call<History>('GET', `/api/chats/${chat}`),
  tasks: (chat: string) => call<{ tasks: Task[]; waits: Wait[] }>('GET', `/api/chats/${chat}/tasks`),
  /** The person's open questions, and agents' stale tasks across chats. */
  personTasks: () => call<{ tasks: PersonTask[]; stale: StaleTask[] }>('GET', '/api/person-tasks'),
  cancelTask: (task: number) => call<unknown>('POST', `/api/tasks/${task}/cancel`, {}),
  restartTask: (task: number) => call<unknown>('POST', `/api/tasks/${task}/restart`),
  search: (q: string) =>
    call<{ chats: ChatHit[]; messages: MessageHit[] }>('GET', `/api/search?q=${encodeURIComponent(q)}`),
  settings: () => call<Settings>('GET', '/api/settings'),
  saveSettings: (change: Partial<Settings>) => call<Settings>('PATCH', '/api/settings', change),
  answer: (answers: PersonAnswer[]) =>
    call<{ answered: Task[]; refused: { task: number; why: string; ref: string | null }[] }>(
      'POST',
      '/api/person-tasks/answer',
      { answers },
    ),
  send: (chat: string, text: string, agent?: string) =>
    call<{ turn: string; status: TurnStatus }>('POST', `/api/chats/${chat}/messages`, { text, agent }),
  pauses: () => call<Pauses>('GET', '/api/pauses'),
  // Each answers every pause as it is then.
  pause: (scope: PauseScope) => call<Pauses>('POST', '/api/pauses', scope),
  resume: (scope: PauseScope) => call<Pauses>('POST', '/api/pauses/resume', scope),
  /** Stop now: ends the scope's turns and the commands they started; carry-ons wait for Restart. */
  stop: (scope: PauseScope) => call<Pauses>('POST', '/api/pauses/stop', scope),
  approve: (chat: string, request: string, allow: boolean) =>
    call<unknown>('POST', `/api/chats/${chat}/approvals/${request}`, { allow }),
  turns: (chat: string) => call<TurnTimeline[]>('GET', `/api/chats/${chat}/turns`),
  engine: (engine: string) => call<Engine>('GET', `/api/engines/${engine}`),
  checkEngine: (engine: string) => call<unknown>('POST', `/api/engines/${engine}/update`),
  accounts: () => call<{ accounts: Account[] }>('GET', '/api/accounts').then((a) => a.accounts),
  addAccount: (engine: string, name: string) => call<Account>('POST', '/api/accounts', { engine, name }),
  renameAccount: (account: string, name: string) =>
    call<Account>('PATCH', `/api/accounts/${account}`, { name }),
  removeAccount: (account: string) => call<unknown>('DELETE', `/api/accounts/${account}`),
  signOut: (account: string) => call<unknown>('POST', `/api/accounts/${account}/sign-out`),
  /** With `device`, a device code instead of the browser page (0033). */
  signIn: (account: string, device?: boolean) =>
    call<unknown>('POST', `/api/accounts/${account}/sign-in`, device ? { device } : undefined),
  signInCode: (account: string, code: string) =>
    call<unknown>('POST', `/api/accounts/${account}/sign-in/code`, { code }),
  cancelSignIn: (account: string) => call<unknown>('DELETE', `/api/accounts/${account}/sign-in`),
  models: (account: string) =>
    call<{ models: Model[] }>('GET', `/api/accounts/${account}/models`).then((m) => m.models),
  /** Whether this computer opens Gathr at login, as the system says now. */
  startup: () => call<{ on: boolean }>('GET', '/api/startup').then((s) => s.on),
  setStartup: (on: boolean) => call<{ on: boolean }>('PUT', '/api/startup', { on }).then((s) => s.on),
  agents: () => call<{ agents: Agent[] }>('GET', '/api/agents').then((a) => a.agents),
  addAgent: (agent: Omit<Agent, 'id'>) => call<Agent>('POST', '/api/agents', agent),
  saveAgent: ({ id, ...agent }: Agent) => call<Agent>('PATCH', `/api/agents/${id}`, agent),
  removeAgent: (agent: string) => call<unknown>('DELETE', `/api/agents/${agent}`),
  /** A reference's events, as `gathr diag trace` shows them, in a turn's timeline shape. */
  trace: (ref: string) =>
    call<{ events: TraceRow[] }>('GET', `/api/trace/${ref}`).then(({ events }) =>
      events.map(
        (e): TurnEvent => ({
          at_ms: e.offset_ms,
          name: e.name,
          phase: e.phase,
          status: e.status,
          error: e.error_type,
          duration_ms: e.duration_us === null ? null : e.duration_us / 1000,
          attrs: e.attrs,
        }),
      ),
    ),
};

interface TraceRow {
  offset_ms: number;
  name: string;
  phase: TurnEvent['phase'];
  status: TurnEvent['status'];
  error_type: string | null;
  duration_us: number | null;
  attrs: Record<string, unknown> | null;
}

export interface TurnEvent {
  at_ms: number;
  name: string;
  phase: 'start' | 'end' | 'point';
  status: 'ok' | 'error' | null;
  error: string | null;
  duration_ms: number | null;
  attrs: Record<string, unknown> | null;
}

export interface TurnTimeline extends Turn {
  n: number;
  asked: string | null;
  created: string;
  ended: string | null;
  events: TurnEvent[];
}
