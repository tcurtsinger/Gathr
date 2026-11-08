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
  role: 'user' | 'assistant';
  kind: string;
  body: string | ToolBody;
}

export interface Turn {
  id: string;
  status: TurnStatus;
  why: string | null;
  ref: string;
}

export interface Chat {
  id: string;
  title: string;
}

export interface History {
  chat: string;
  messages: Message[];
  turns: Turn[];
  /** Questions the engine asked that nobody has answered yet. */
  approvals: { turn: string; request_id: string; tool: string }[];
}

export interface Engine {
  found: string | null;
  why_not_found: string | null;
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
  createChat: () => call<Chat>('POST', '/api/chats', {}),
  history: (chat: string) => call<History>('GET', `/api/chats/${chat}`),
  send: (chat: string, text: string) =>
    call<{ turn: string; status: TurnStatus }>('POST', `/api/chats/${chat}/messages`, { text }),
  stop: (chat: string) => call<unknown>('POST', `/api/chats/${chat}/stop`),
  approve: (chat: string, request: string, allow: boolean) =>
    call<unknown>('POST', `/api/chats/${chat}/approvals/${request}`, { allow }),
  turns: (chat: string) => call<TurnTimeline[]>('GET', `/api/chats/${chat}/turns`),
  engine: () => call<Engine>('GET', '/api/engines/claude'),
  account: (account: string) => call<Account>('GET', `/api/accounts/${account}`),
  signIn: (account: string) => call<{ url: string }>('POST', `/api/accounts/${account}/sign-in`),
  signInCode: (account: string, code: string) =>
    call<unknown>('POST', `/api/accounts/${account}/sign-in/code`, { code }),
  cancelSignIn: (account: string) => call<unknown>('DELETE', `/api/accounts/${account}/sign-in`),
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
