// The service's API (AGENTS.md "The API"). The page is served from the same origin as /api:
// by the Vite proxy in dev, which adds the token.

export type TurnStatus = 'queued' | 'waiting_sign_in' | 'running' | 'done' | 'stopped' | 'failed';

export interface ToolBody {
  tool: string;
  status: 'running' | 'ok' | 'error' | 'stopped';
  duration_ms?: number;
}

export interface Message {
  id: string;
  turn: string | null;
  time: string;
  /** `gathr`: what Gathr told an agent (a task, a wake-up, a hear-back), never the agent's own words. */
  role: 'user' | 'assistant' | 'gathr';
  kind: string;
  body: string | ToolBody;
}

export interface Turn {
  id: string;
  status: TurnStatus;
  why: string | null;
  ref: string;
  /** The agent it was sent to; null for a turn sent to no agent. */
  agent: string | null;
  created?: string;
  /** The task it works on: its reply sits in the thread under the turn that gave the task. */
  task?: number | null;
  /** A hear-back's: the turn whose tasks it reports on, which its reply links back to. */
  answers?: string | null;
}

export type Role = 'supervisor' | 'reviewer' | 'member';

export interface Member {
  agent: string;
  name: string;
  role: Role;
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

/** A task an agent gave another (0010). */
export interface Task {
  number: number;
  chat: string;
  /** The agent who gave it; null when the person did. */
  giver: string | null;
  owner: string;
  /** The turn that gave it. */
  given_in: string | null;
  kind: 'work' | 'review';
  what: string;
  state: TaskState;
  reason: string | null;
  /** Tasks it waits for before it starts. */
  after: number[];
}

/** What an agent waits on (0010): another agent, CI, a PR or a check. */
export interface Wait {
  id: string;
  agent: string;
  task: number | null;
  kind: string;
  target: string | null;
  state: string;
}

export interface Project {
  id: string;
  name: string;
  folder: string;
  members: Member[];
}

export interface Chat {
  id: string;
  title: string;
  /** Its project's id; none for a plain chat. */
  project?: string | null;
  folder?: string | null;
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
  approvals: { turn: string; request_id: string; tool: string }[];
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
  /** A sign-in from Settings: waiting (with its page once there is one), or failed with why and a reference. */
  sign_in: { state: 'waiting'; url?: string } | { state: 'failed'; why: string; ref: string } | null;
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
    throw new Failure('Connection failed', 'the service did not answer', null);
  }
  const answer = await response.json().catch(() => null);
  if (!response.ok || answer === null) {
    const why =
      answer?.why ?? (answer === null ? `the answer to ${path} wasn't JSON (${response.status})` : null);
    throw new Failure(answer?.error ?? 'Request failed', why, answer?.ref ?? null);
  }
  return answer as T;
}

export const api = {
  chats: () => call<Chat[]>('GET', '/api/chats'),
  createChat: (project?: string) => call<Chat>('POST', '/api/chats', project ? { project } : {}),
  projects: () => call<{ projects: Project[] }>('GET', '/api/projects').then((p) => p.projects),
  addProject: (project: { name: string; folder: string; members: { agent: string; role: Role }[] }) =>
    call<Project>('POST', '/api/projects', project),
  history: (chat: string) => call<History>('GET', `/api/chats/${chat}`),
  tasks: (chat: string) => call<{ tasks: Task[]; waits: Wait[] }>('GET', `/api/chats/${chat}/tasks`),
  send: (chat: string, text: string, agent?: string) =>
    call<{ turn: string; status: TurnStatus }>('POST', `/api/chats/${chat}/messages`, { text, agent }),
  stop: (chat: string) => call<unknown>('POST', `/api/chats/${chat}/stop`),
  approve: (chat: string, request: string, allow: boolean) =>
    call<unknown>('POST', `/api/chats/${chat}/approvals/${request}`, { allow }),
  turns: (chat: string) => call<TurnTimeline[]>('GET', `/api/chats/${chat}/turns`),
  engine: () => call<Engine>('GET', '/api/engines/claude'),
  checkEngine: () => call<unknown>('POST', '/api/engines/claude/update'),
  accounts: () => call<{ accounts: Account[] }>('GET', '/api/accounts').then((a) => a.accounts),
  addAccount: (engine: string, name: string) => call<Account>('POST', '/api/accounts', { engine, name }),
  renameAccount: (account: string, name: string) =>
    call<Account>('PATCH', `/api/accounts/${account}`, { name }),
  removeAccount: (account: string) => call<unknown>('DELETE', `/api/accounts/${account}`),
  signOut: (account: string) => call<unknown>('POST', `/api/accounts/${account}/sign-out`),
  signIn: (account: string) => call<{ url: string }>('POST', `/api/accounts/${account}/sign-in`),
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
