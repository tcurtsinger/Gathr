// The service's API (AGENTS.md "The API"). The page is served from the same origin as /api:
// by the Vite proxy in dev, which adds the token.
import type { Block } from './block';
import type { Added } from './components/AddedBy';
import type { Deletable, DeleteCounts } from './deleting';
import type { MemoryPlace, MemoryShown } from './memory/places';
import type { Schedule, ScheduleShown } from './scheduled/schedule';
import type { Connect } from './settings/Connect';

export type { Block, Cell } from './block';

export type TurnStatus = 'queued' | 'waiting_sign_in' | 'running' | 'done' | 'stopped' | 'failed';

export interface ToolBody {
  tool: string;
  status: 'running' | 'ok' | 'error' | 'stopped';
  duration_ms?: number;
  /**
   * What it ran (0041): a command's first line, a file's path, a search's words, with secrets hidden, and
   * the engine's own words for it when it gave some. Tools that keep nothing, and messages from before, have none.
   */
  input?: {
    text: string;
    title?: string;
    /** A command's whole text when it says more than `text` (0044): up to 8 lines, secrets hidden. */
    request?: string;
  };
  /** What came back (0044): up to 5 lines, secrets hidden; how many lines there were, and whether more followed. */
  result?: { text: string; lines: number; more: boolean };
  /**
   * The files it changed, as the engine reported them: `path` from the engine's folder when it is there, `full` the
   * whole path when that's another, and the lines added and removed. None from before they were kept.
   */
  changed?: { path: string; full?: string; added: number; removed: number }[];
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
  /** `text`: its body is the words. `tool`, `thinking` and `block`: its body is an object. */
  kind: string;
  body: string | ToolBody | ThinkingBody | Block;
  /** A message an agent sent one member: that member's id. It shows as its own block (0037). */
  to?: string | null;
  /** What Gathr told the member in its turn: the message it carries, already shown as the sender's. */
  copy_of?: string | null;
  /** The person's message: the names they picked from the "@" list, drawn blue. None from before they were kept. */
  picks?: PickedName[] | null;
  /** A line Gathr said in no turn: the main message it sits under (0075). */
  topic?: string | null;
  /** The person's reply in a thread (0076): the message it answers. */
  reply_to?: string | null;
  /** The person's reply about a task: a task's message has no message of its own. */
  reply_task?: number | null;
  /** Gathr's line a compaction left, "Compacted Ann's conversation: 468K to 52K": the one line of Gathr's shown. */
  compacted?: boolean;
  /** A scheduled run's words, drawn as the person's: when they were due. */
  scheduled?: string;
  /** A project's brief, drawn as the person's: the assistant that wrote it. */
  from?: string;
  /** Words an AI app outside Gathr sent for the person, drawn under the app's logo and brand: the name it gave. */
  app?: string;
}

/** A command an agent's engine still runs in the background: who started it, what it is, since when. */
export interface Background {
  agent: string | null;
  description: string;
  started: string;
}

/** What a reply in a thread answers (0076): a message, or a task. */
export interface ReplyTarget {
  reply_to?: string;
  reply_task?: number;
}

/** A name picked from the "@" list in the text: where its `@Name` starts and how long it is, in UTF-16 units. */
export interface PickedName {
  at: number;
  len: number;
  agent: string;
}

/**
 * Who or what stopped a turn (0051): the person (`you`), a restart, or Gathr for a reason of its own: its task waits
 * for its plan, its task was cancelled, the account ran out of usage, or anything else (`gathr`).
 */
export type TurnEnd = 'you' | 'restart' | 'plan' | 'cancelled' | 'usage' | 'gathr';

export interface Turn {
  id: string;
  status: TurnStatus;
  why: string | null;
  /** A stopped turn's: who or what stopped it. */
  end?: TurnEnd | null;
  ref: string;
  /** The agent it was sent to; null for a turn sent to no agent. */
  agent: string | null;
  created?: string;
  /** When it ended; none while it is to end. Its name line says how long it took (0082). */
  ended?: string | null;
  /** Its place in the chat by when it started; none until it starts. */
  position?: number | null;
  /** The task it works on: its reply sits in the thread under the turn that gave the task. */
  task?: number | null;
  /** A hear-back's: the turn whose tasks it reports on, which its reply links back to. */
  answers?: string | null;
  /** Queued but held by a pause, usage, an app update or its first engine download. */
  held?: 'pause' | 'usage' | 'update' | 'claude' | 'codex' | null;
  /** The main message it sits under (0075): `t:<task>`, `m:<message>`, `r:<turn>` (a result), `b:<turn>` (its own). */
  topic?: string | null;
  /** The ask it is the result of (0051): the result quotes it. */
  result_of?: string | null;
  /** The kind of Gathr's words that opened it (0056): `check` and `last_word` are the open check's (0080). */
  opened?: string | null;
}

export type Role = 'supervisor' | 'member';

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
  /** Where its engine compacts by itself (Auto-compact, capped at 80% of the window); null for the engine's own. */
  compact_at?: number | null;
}

/** The whole app, or one chat. */
export type PauseScope = { scope: 'app' } | { scope: 'chat'; target: string };

/**
 * A pause (0010): pausing while a turn, background command or wait's check in it still runs, paused once quiet,
 * stopped after Stop now until Restart (its resume).
 */
export interface Pause {
  scope: 'app' | 'chat';
  /** The chat's id; empty for the app. */
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

/** Friction an agent reported: who, on which task, in which chat and where. A deleted chat or agent leaves null. */
export interface FrictionReport {
  id: number;
  time: string;
  chat: string;
  chat_title: string | null;
  agent: string | null;
  agent_name: string | null;
  task: number | null;
  what: string;
  where: string | null;
}

export interface Project extends Added {
  id: string;
  name: string;
  folder: string;
  members: Member[];
}

/** What a project's settings change: only what it names; the team whole. */
export type ProjectChange = Partial<Pick<Project, 'name' | 'folder'>> & {
  members?: { agent: string; role: Role }[];
  /** Make the folder a git repository first when it isn't in one: New project's "Set up git and create". */
  init?: boolean;
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
  /** The most pressing of its agents' marks its row shows (0071): none while nothing of those three holds. */
  mark?: ChatMark | null;
  /** The assistant's own chat: the sidebar shows it as the assistant's row, in no list. */
  assistant?: boolean;
}

/** A quiet line of the assistant's chat: an agent joined it or left it. */
export interface Stay {
  agent: string;
  change: 'joined' | 'left';
  time: string;
}

export type ChatMark = 'needs_you' | 'cant_run' | 'working';

/** A piece of work on a chat's board, set by its agents. `owner` is an agent's id, `name` its name. */
export interface BoardLine {
  id: number;
  owner: string;
  name: string;
  text: string;
  state: 'to_do' | 'doing' | 'in_review' | 'done';
}

/** A folder at the top of a chat's tree: the project's own, Shared, or an agent's for this chat. */
export interface FilePart {
  name: string;
  path: string;
  agent?: string;
}
export interface FileTree {
  project: FilePart | null;
  shared: FilePart | null;
  /** The project's part of Designs: each folder in it is a design. */
  designs: FilePart | null;
  agents: FilePart[];
}
export interface FileEntry {
  name: string;
  path: string;
  folder: boolean;
  /** When it last changed, in milliseconds since 1970. */
  modified?: number | null;
}
/** A design: a folder of HTML pages, one per option, its pages in their names' order. */
export interface Design {
  name: string;
  path: string;
  pages: string[];
  /** When its newest page changed, in milliseconds since 1970. */
  modified: number;
}
/** A part of Designs: a project's, with its newest chat that isn't archived, or Chats. Its designs newest first. */
export interface DesignPart {
  folder: string;
  project: { id: string; name: string } | null;
  chat: string | null;
  designs: Design[];
}
/** A file an agent changed in its worktree: its path there, what it was before a rename, and its lines added and removed. */
export interface ChangedFile {
  path: string;
  from?: string;
  added: number;
  removed: number;
  kind: 'changed' | 'new' | 'deleted' | 'renamed';
  binary: boolean;
}
/**
 * What each agent changed in the chat: in its worktree (with its branch) or, in no checkout, the folder its engine runs
 * in; and in each other git checkout or folder it wrote in (`outside`, by that folder); `more` when there were more
 * files than are listed.
 */
export interface Changes {
  agents: {
    id: string;
    name: string;
    branch: string | null;
    files: ChangedFile[];
    outside: { folder: string; files: ChangedFile[] }[];
  }[];
  more: boolean;
}
export interface ChangeLine {
  kind: 'context' | 'added' | 'removed';
  old?: number;
  new?: number;
  text: string;
}
/** One file's change: its hunks, each with its old and new range as [start, count]; `cut` past its first 2,000 lines. */
export interface FileChange {
  binary: boolean;
  cut: boolean;
  hunks: { old: [number, number]; new: [number, number]; lines: ChangeLine[] }[];
}
/** Why a file isn't shown as text: it's gone, too large, or not text. */
export type Unshown = 'gone' | 'large' | 'cant';
const UNSHOWN: Record<number, Unshown> = { 404: 'gone', 413: 'large', 415: 'cant' };

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

/** Settings > General: the folder that holds every project's Shared and agent folders (M1), and the rest. */
export interface Settings {
  agent_folder: string;
  /** Messages between agents in a row before the chat pauses; null for no limit. */
  handoff_limit: number | null;
  /** Tokens an engine compacts at (200000 to 500000); null for each engine's own. */
  auto_compact: number | null;
  /** What agents call the person; null until set. */
  person_name?: string | null;
  /** The agent a new chat's message that names nobody goes to: the one picked, else the first; null with no agents. */
  assistant?: string | null;
  /** The background model the person picked; null for Gathr's own pick. */
  background_model?: { account: string; model: string } | null;
  /** Settings > Connect: what an AI app on this PC runs; null with no command line here. Read only. */
  connect?: Connect | null;
}

/** A model the background can run on: one of a signed-in account's. */
export interface BackgroundChoice {
  account: string;
  account_name: string;
  model: string;
  name: string;
}

/** A reusable agent (0026): its account implies the engine. Null model or effort is the engine's default. */
export interface Agent extends Added {
  id: string;
  name: string;
  account: string;
  model: string | null;
  effort: string | null;
  /** What it may do without asking (0029). */
  approvals: Approvals;
  /** Its picture's shape, eyes and colour by name; null until the person picks, worked out from the id. */
  shape: string | null;
  eyes: string | null;
  colour: string | null;
  /** One line of up to 80 characters: what it is for, beside its name in the "@" list. */
  description?: string | null;
}

/** An agent as Settings saves it: a picture field is sent only when it changed. */
export type AgentDraft = Omit<Agent, 'id' | 'shape' | 'eyes' | 'colour'> &
  Partial<Pick<Agent, 'shape' | 'eyes' | 'colour'>>;

/** A removed agent: its old messages still say who wrote them, with its picture (0069). */
export type Named = Pick<Agent, 'id' | 'name' | 'shape' | 'eyes' | 'colour'>;

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
  approvals: { turn: string; request_id: string; tool: string; input?: unknown; why?: string | null }[];
  /** The chat's agents and their roles (0028); empty for a chat nobody joined yet. */
  members: Member[];
  /** Replies still streaming, so a chat opened mid-turn shows what arrived so far. */
  streaming?: { turn: string; agent: string | null; text: string; reply_to?: string | null }[];
  /** Agents' thumbs-ups, oldest first. */
  reactions?: Reaction[];
  /** A room chat's questions to the person, oldest first. */
  questions?: Question[];
  /** The commands its agents' engines still run in the background, oldest first. */
  background?: Background[];
  /** The assistant's chat's quiet lines, oldest first; none in another chat. */
  stays?: Stay[];
}

/**
 * An agent's question to the person in a room chat: open until `answered`, then with their pick, words or both, or
 * until `closed` unanswered when the chat was archived.
 */
export interface Question {
  id: number;
  chat: string;
  agent: string;
  name: string;
  question: string;
  choices: { label: string; detail: string | null }[];
  pick: string | null;
  words: string | null;
  asked: string;
  answered: string | null;
  closed: string | null;
}

/**
 * A thumbs-up on an agent's message, the original's id, never Gathr's copy: another agent's, or the person's like
 * (`agent` is `person`, 0057).
 */
export interface Reaction {
  message: string;
  agent: string;
  time: string;
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

/** An engine with no copy yet says so in a why: it is downloading, not failing. */
export const downloading = (why?: string | null) => !!why?.endsWith(' is downloading');

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
/** Gathr's own version and update, as `GET /api/app` answers (0032). */
export interface AppState {
  version: string;
  /** No release feed (a dev build), nothing to do, a check under way, a release ready, or installing it. */
  state: 'off' | 'idle' | 'checking' | 'ready' | 'updating';
  /** The release checked and ready to install. */
  update: { version: string } | null;
  /** The last check or update that failed. */
  failed: { why: string; ref: string } | null;
}

/** A folder in the Skills folder: its SKILL.md's description, or why it can't be read. */
export interface Skill extends Added {
  name: string;
  description: string | null;
  path: string;
  enabled: boolean;
  problem: string | null;
}

const memoryPath = (place: string) => `/api/memory/${encodeURIComponent(place)}`;

/** A tool on the web, or on this PC (`kind: 'pc'`, no link): never its key or a variable's value. */
export interface WebTool extends Added {
  id: string;
  name: string;
  link: string;
  kind: 'web' | 'pc';
  enabled: boolean;
  has_key: boolean;
  /** Whether it signed in through the browser: never the sign-in. */
  signed_in: boolean;
  /** The Client ID it signs in with, where its sign-in service takes no app it doesn't know: never the secret. */
  client_id: string | null;
  /**
   * Asked of it when listed; none while it's disabled. A tool on this PC is connected while it runs. `needs_key`: an
   * agent added it without one.
   */
  state:
    | 'connected'
    | 'not_reachable'
    | 'key_refused'
    | 'needs_key'
    | 'sign_in'
    | 'starting'
    | 'not_started'
    | null;
}

/** What a tool on this PC runs: the program found (none when it isn't), as typed, and its variables' names. */
export interface PcRuns {
  program: string | null;
  typed: string;
  args: string[];
  folder: string | null;
  variables: string[];
  /** Why the program can't run, when it isn't found. */
  missing: string | null;
}

/** Pasted text read back before anything runs: its name, and where the program comes from when that's known. */
export interface PcRead extends PcRuns {
  name: string;
  from: string | null;
}

/** A tool whose sign-in service takes no app it doesn't know: the address its admin gives the app they make. */
export interface NeedsClient {
  registers?: false;
  redirect?: string;
}

/** The Client ID an admin made for Gathr, and its secret if any. */
export interface ClientGiven {
  client_id?: string;
  client_secret?: string;
}

/** One thing a tool lets an agent do, with the first line of its description. */
export interface ToolAction {
  name: string;
  description: string | null;
}

export class Failure extends Error {
  constructor(
    readonly error: string,
    readonly why: string | null,
    readonly ref: string | null,
    /**
     * The service didn't answer at all: it's starting, or handing over to an update. What only loads says
     * nothing then: the live socket's return reloads it, so there's nothing to tell the person or to dismiss.
     */
    readonly away = false,
  ) {
    super(why ? `${error}: ${why}` : error);
  }
}

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  let response: Response;
  try {
    // A file goes as it is; anything else as JSON.
    const raw = body instanceof Blob;
    response = await fetch(path, {
      method,
      headers: body === undefined || raw ? {} : { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : raw ? body : JSON.stringify(body),
    });
  } catch {
    throw new Failure('Connection failed', 'No response. Try again.', null, true);
  }
  const answer = await response.json().catch(() => null);
  if (!response.ok || answer === null) throw refusal(path, response, answer);
  return answer as T;
}

function refusal(
  path: string,
  response: Response,
  answer: { error?: string; why?: string; ref?: string } | null,
) {
  const why = answer?.why ?? (answer === null ? `${path}: invalid JSON (HTTP ${response.status})` : null);
  return new Failure(answer?.error ?? 'Request failed', why, answer?.ref ?? null);
}

/** The service's answer for a file's tab, at `/api/chats/<chat>/<what>`, or why the file isn't shown. */
async function shown(chat: string, what: string, file: string): Promise<Response | { unshown: Unshown }> {
  const path = `/api/chats/${chat}/${what}?path=${encodeURIComponent(file)}`;
  let response: Response;
  try {
    response = await fetch(path);
  } catch {
    throw new Failure('Connection failed', 'No response. Try again.', null, true);
  }
  if (response.ok) return response;
  const unshown = UNSHOWN[response.status];
  if (unshown) return { unshown };
  throw refusal(path, response, await response.json().catch(() => null));
}

/** A file's words, or why they aren't shown. */
async function words(chat: string, file: string): Promise<{ text: string } | { unshown: Unshown }> {
  const got = await shown(chat, 'file', file);
  return got instanceof Response ? { text: await got.text() } : got;
}

/** A PDF's bytes, or why they aren't shown. */
async function pdf(chat: string, file: string): Promise<{ bytes: ArrayBuffer } | { unshown: Unshown }> {
  const got = await shown(chat, 'pdf', file);
  return got instanceof Response ? { bytes: await got.arrayBuffer() } : got;
}

/** A workbook's sheet: its first rows (the heading row and the 2,000 a table draws) and how many it has in all. */
export interface Sheet {
  name: string;
  rows: string[][];
  total: number;
}

/** A workbook's visible sheets, or why they aren't shown. */
async function sheets(chat: string, file: string): Promise<{ sheets: Sheet[] } | { unshown: Unshown }> {
  const got = await shown(chat, 'sheets', file);
  return got instanceof Response ? ((await got.json()) as { sheets: Sheet[] }) : got;
}

/** Saves each file in the chat's folder, in order: their saved names. */
async function upload(chat: string, files: readonly File[]) {
  const saved: string[] = [];
  for (const file of files) {
    const path = `/api/chats/${chat}/files?name=${encodeURIComponent(file.name)}`;
    saved.push((await call<{ name: string }>('POST', path, file)).name);
  }
  return saved;
}

export const api = {
  chats: () => call<Chat[]>('GET', '/api/chats'),
  archivedChats: () => call<Chat[]>('GET', '/api/chats?archived=true'),
  /** Ends the chat's work in one step; Restore (unarchive) never starts it again. */
  archive: (chat: string) => call<unknown>('POST', `/api/chats/${chat}/archive`),
  unarchive: (chat: string) => call<unknown>('POST', `/api/chats/${chat}/unarchive`),
  deleteChat: (chat: string) => call<unknown>('DELETE', `/api/chats/${chat}`),
  /** With `assistant`, the assistant's own chat: the one there is, if there is. */
  createChat: (project?: string, assistant?: boolean) =>
    call<Chat>('POST', '/api/chats', assistant ? { assistant } : project ? { project } : {}),
  projects: () => call<{ projects: Project[] }>('GET', '/api/projects').then((p) => p.projects),
  addProject: (project: {
    name: string;
    folder: string;
    members: { agent: string; role: Role }[];
    init?: boolean;
  }) => call<Project>('POST', '/api/projects', project),
  /** Whether a folder is there and in a git repository: New project's Folder row. */
  checkFolder: (folder: string) =>
    call<{ folder: boolean; repo: boolean }>('POST', '/api/projects/check', { folder }),
  /** Make project, from the card of a project the assistant proposed: as proposed, or as the New project form says. */
  makeProject: (message: string, change?: ProjectChange) =>
    call<{ project: string; chat: string; turn: string; message: Message }>(
      'POST',
      `/api/proposals/${message}/make`,
      change ?? {},
    ),
  saveProject: (id: string, change: ProjectChange) => call<Project>('PATCH', `/api/projects/${id}`, change),
  history: (chat: string) => call<History>('GET', `/api/chats/${chat}`),
  board: (chat: string) => call<{ lines: BoardLine[]; agents: number }>('GET', `/api/chats/${chat}/board`),
  /** The chat's tree: its top folders. */
  tree: (chat: string) => call<FileTree>('GET', `/api/chats/${chat}/files`),
  /** One folder of the chat's tree: folders first, then files. */
  folder: (chat: string, path: string) =>
    call<{ entries: FileEntry[] }>('GET', `/api/chats/${chat}/files?path=${encodeURIComponent(path)}`).then(
      (f) => f.entries,
    ),
  /** What each agent changed in the chat: in its own worktree, and in other checkouts it wrote in. */
  changes: (chat: string) => call<Changes>('GET', `/api/chats/${chat}/changes`),
  /** One file's change in an agent's worktree, or in the checkout at `folder`, as lines. */
  change: (chat: string, agent: string, file: ChangedFile, folder?: string) => {
    const q = new URLSearchParams({
      agent,
      path: file.path,
      ...(file.from ? { from: file.from } : {}),
      ...(folder ? { folder } : {}),
    });
    return call<FileChange>('GET', `/api/chats/${chat}/change?${q}`);
  },
  /** The chat's files whose name holds the words, best first: each with its place from the top folder down. */
  find: (chat: string, words: string) =>
    call<{ found: { path: string; crumbs: string[] }[] }>(
      'GET',
      `/api/chats/${chat}/files?find=${encodeURIComponent(words)}`,
    ).then((f) => f.found),
  words,
  pdf,
  sheets,
  /**
   * A view of an HTML page in the chat's folders: the address its sealed frame loads it and the files beside it at.
   * With `notes`, for a design's tab: a page of a design only, served with the script for notes.
   */
  view: (chat: string, path: string, notes = false) =>
    call<{ address: string }>(
      'POST',
      `/api/chats/${chat}/view?path=${encodeURIComponent(path)}${notes ? '&notes=true' : ''}`,
    ).then((v) => v.address),
  /** Every design, by its part of Designs, the part with the newest design first. */
  designs: () => call<{ parts: DesignPart[] }>('GET', '/api/designs').then((d) => d.parts),
  /** A view of a page in Designs, as `view` is of a chat's: one per design while the service keeps it. */
  designView: (path: string) =>
    call<{ address: string }>('POST', `/api/designs/view?path=${encodeURIComponent(path)}`).then(
      (v) => v.address,
    ),
  search: (q: string) =>
    call<{ chats: ChatHit[]; messages: MessageHit[] }>('GET', `/api/search?q=${encodeURIComponent(q)}`),
  settings: () => call<Settings>('GET', '/api/settings'),
  saveSettings: (change: Partial<Settings>) => call<Settings>('PATCH', '/api/settings', change),
  /** Gathr's own pick for the background model (null when no account has a small one), and every choice. */
  background: () =>
    call<{ auto: BackgroundChoice | null; choices: BackgroundChoice[] }>('GET', '/api/settings/background'),
  /** The person's name for a chat, which the background model never replaces. */
  renameChat: (chat: string, title: string) =>
    call<{ chat: string; title: string }>('PATCH', `/api/chats/${chat}`, { title }),
  /** Sends the message, with the files first saved in the chat's folder, a line each after its words. */
  send: async (
    chat: string,
    text: string,
    agent?: string,
    picks?: PickedName[],
    reply?: ReplyTarget,
    files: readonly File[] = [],
  ) => {
    const saved = await upload(chat, files);
    return call<{ turn: string; status: TurnStatus }>('POST', `/api/chats/${chat}/messages`, {
      text,
      agent,
      picks: picks?.length ? picks : undefined,
      files: saved.length ? saved : undefined,
      ...reply,
    });
  },
  /** Compacts the agent's conversation in the chat, once its turn and background commands are done. */
  compact: (chat: string, agent: string) =>
    call<{ turn: string; status: TurnStatus }>('POST', `/api/chats/${chat}/agents/${agent}/compact`, {}),
  /** The person's like (0057): it reaches the message's agent. */
  like: (chat: string, message: string) =>
    call<{ reaction: Reaction; did: string }>('PUT', `/api/chats/${chat}/messages/${message}/like`, {}),
  /** Answers a room chat's question: it starts the asking agent's turn. */
  answerQuestion: async (question: Question, pick?: string, words?: string, files: readonly File[] = []) => {
    const saved = await upload(question.chat, files);
    return call<{ question: Question; turn: string }>('POST', `/api/questions/${question.id}/answer`, {
      pick,
      words,
      files: saved.length ? saved : undefined,
    });
  },
  /** Takes the like back: the mark goes, and nothing it did is undone. */
  unlike: (chat: string, message: string) =>
    call<{ removed: boolean }>('DELETE', `/api/chats/${chat}/messages/${message}/like`),
  pauses: () => call<Pauses>('GET', '/api/pauses'),
  // Each answers every pause as it is then.
  pause: (scope: PauseScope) => call<Pauses>('POST', '/api/pauses', scope),
  resume: (scope: PauseScope) => call<Pauses>('POST', '/api/pauses/resume', scope),
  /** Stop now: ends the scope's turns and the commands they started; carry-ons wait for Restart. */
  stop: (scope: PauseScope) => call<Pauses>('POST', '/api/pauses/stop', scope),
  approve: (chat: string, request: string, allow: boolean) =>
    call<unknown>('POST', `/api/chats/${chat}/approvals/${request}`, { allow }),
  turns: (chat: string) => call<TurnTimeline[]>('GET', `/api/chats/${chat}/turns`),
  /** What agents reported got in their way (0048), newest first. */
  friction: () => call<{ friction: FrictionReport[] }>('GET', '/api/friction').then((f) => f.friction),
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
  app: () => call<AppState>('GET', '/api/app'),
  /** Update: installs the ready release now; what runs is stopped and carries on after. */
  updateApp: () => call<unknown>('POST', '/api/app/update', { install: true }),
  checkApp: () => call<unknown>('POST', '/api/app/update'),
  startup: () => call<{ on: boolean }>('GET', '/api/startup').then((s) => s.on),
  setStartup: (on: boolean) => call<{ on: boolean }>('PUT', '/api/startup', { on }).then((s) => s.on),
  /** The agents, and the removed ones apart, for their old messages (0069). */
  agents: () => call<{ agents: Agent[]; removed: Named[] }>('GET', '/api/agents'),
  addAgent: (agent: AgentDraft) => call<Agent>('POST', '/api/agents', agent),
  /** Add agent, from the card of an agent the assistant proposed: as proposed, or as the Agents screen's form says. */
  addProposedAgent: (message: string, agent?: AgentDraft) =>
    call<{ agent: Agent; message: Message }>('POST', `/api/proposals/${message}/add`, agent ?? {}),
  /** Save, from the card of a change the assistant proposed to an agent or a project. */
  saveProposedChange: (message: string) =>
    call<{ message: Message }>('POST', `/api/proposals/${message}/save`),
  saveAgent: ({ id, ...agent }: AgentDraft & { id: string }) =>
    call<Agent>('PATCH', `/api/agents/${id}`, agent),
  /** What deleting the project or agent would do, for the confirm's words. */
  deleteCounts: (kind: Deletable, id: string) => call<DeleteCounts>('GET', `/api/${kind}s/${id}/delete`),
  /** What it did, and the folders it couldn't remove yet (`left`). */
  delete: (kind: Deletable, id: string) =>
    call<DeleteCounts & { left: string[] }>('DELETE', `/api/${kind}s/${id}`),
  skills: () => call<{ folder: string; skills: Skill[] }>('GET', '/api/skills'),
  /** The skill with its SKILL.md's text after the front matter; none without a SKILL.md. */
  skill: (name: string) =>
    call<Skill & { text: string | null }>('GET', `/api/skills/${encodeURIComponent(name)}`),
  enableSkill: (name: string, enabled: boolean) =>
    call<Skill>('PATCH', `/api/skills/${encodeURIComponent(name)}`, { enabled }),
  /** Deletes its folder. */
  removeSkill: (name: string) => call<unknown>('DELETE', `/api/skills/${encodeURIComponent(name)}`),
  newSkill: (skill: { name: string; description: string; text: string }) =>
    call<Skill>('POST', '/api/skills', skill),
  /** One Markdown file as a skill of its own: named by its front matter, else by the file's name. */
  addSkillFile: (file: File) =>
    call<Skill>('POST', `/api/skills?file=${encodeURIComponent(file.name)}`, file).then((s) => s.name),
  /** A zipped skill folder, such as a Claude `.skill` file, unpacked as a new skill. */
  addSkillZip: (file: File) =>
    call<Skill>('POST', `/api/skills?zip=${encodeURIComponent(file.name)}`, file).then((s) => s.name),
  /** A folder picked with its files, each sent with its path inside it: the skill's name is the folder's. */
  addSkillFolder: async (files: readonly File[]) => {
    const parts = (f: File) => f.webkitRelativePath.split('/');
    // Hidden files, such as a Mac's .DS_Store, are left out.
    const kept = files.filter((f) => !parts(f).some((p) => p.startsWith('.')));
    const name = parts(kept[0] ?? files[0])[0];
    const inner = (f: File) => parts(f).slice(1).join('/');
    kept.sort((a, b) => Number(inner(b) === 'SKILL.md') - Number(inner(a) === 'SKILL.md'));
    for (const [i, file] of kept.entries()) {
      const path = `/api/skills/${encodeURIComponent(name)}/files?path=${encodeURIComponent(inner(file))}`;
      await call<unknown>('POST', i === 0 ? `${path}&new=true` : path, file);
    }
    return name;
  },
  /** Opens its folder in the system's file manager: the desktop window only. */
  openSkill: (name: string) => call<unknown>('POST', `/api/skills/${encodeURIComponent(name)}/open`),
  /** Every place in Memory: About you, each project, each agent. */
  memory: () => call<{ folder: string; places: MemoryPlace[] }>('GET', '/api/memory'),
  /** A place's notes, newest first, and its file. */
  memoryPlace: (place: string) => call<MemoryShown>('GET', memoryPath(place)),
  addNote: (place: string, text: string) => call<MemoryShown>('POST', memoryPath(place), { text }),
  changeNote: (place: string, was: string, text: string) =>
    call<MemoryShown>('PATCH', memoryPath(place), { was, text }),
  removeNote: (place: string, text: string) => call<MemoryShown>('DELETE', memoryPath(place), { text }),
  /** Everything scheduled in every chat, oldest first. */
  schedules: () => call<{ schedules: Schedule[] }>('GET', '/api/schedules').then((s) => s.schedules),
  /** One schedule with its last runs, newest first. */
  schedule: (id: string) => call<ScheduleShown>('GET', `/api/schedules/${id}`),
  enableSchedule: (id: string, enabled: boolean) =>
    call<Schedule>('PATCH', `/api/schedules/${id}`, { enabled }),
  deleteSchedule: (id: string) => call<unknown>('DELETE', `/api/schedules/${id}`),
  /** A run at once: the next time stays. */
  runSchedule: (id: string) => call<{ chat: string; turn: string }>('POST', `/api/schedules/${id}/run`),
  /** Every tool on the web, each enabled one's state asked of it now. */
  tools: () => call<{ tools: WebTool[] }>('GET', '/api/tools').then((t) => t.tools),
  /** One tool, with its actions when it's connected; with a Client ID, the address its admin gives that app. */
  tool: (id: string) =>
    call<
      WebTool & { actions: ToolAction[] | null; redirect?: string } & Partial<PcRuns> & {
          printed?: string | null;
          /** A tool on this PC: its variables that have no value yet, by name. */
          empty?: string[];
        }
    >('GET', `/api/tools/${id}`),
  /** What Gathr would run for text pasted from a tool's instructions. Nothing runs. */
  readTool: (text: string) => call<PcRead>('POST', '/api/tools/read', { text }),
  /** Saves a tool on this PC and starts it: saved even when it doesn't start. */
  addPcTool: (tool: { text: string; name?: string }) => call<{ tool: WebTool }>('POST', '/api/tools', tool),
  /** What a tool on this PC runs, pasted again, and its name: it starts again. */
  changePcTool: (id: string, change: { text: string; name?: string }) =>
    call<WebTool>('PATCH', `/api/tools/${id}`, change),
  /** Starts again a tool on this PC that didn't start. */
  startTool: (id: string) => call<{ state: WebTool['state'] }>('POST', `/api/tools/${id}/start`),
  /**
   * Gathr connects to the link: the tool saved, or what it needs first. One that signs in where Gathr can't register
   * itself says so, with the address its admin gives the app they make for Gathr.
   */
  addTool: (tool: { link: string; name?: string; key?: string }) =>
    call<{ tool?: WebTool; needs?: 'key' | 'sign_in' } & NeedsClient>('POST', '/api/tools', tool),
  /** A new key is saved only when the tool takes it. */
  changeTool: (id: string, change: { enabled?: boolean; key?: string }) =>
    call<WebTool>('PATCH', `/api/tools/${id}`, change),
  removeTool: (id: string) => call<unknown>('DELETE', `/api/tools/${id}`),
  /**
   * Opens the tool's sign-in page in the browser; its end comes live as `tools` with this `sign_in`. A Client ID given
   * is used, and kept once signed in; a tool signing in again without one uses its own. One that needs a Client ID
   * and has none answers so instead.
   */
  signInTool: (tool: ({ link: string; name?: string } | { tool: string }) & ClientGiven) =>
    call<{ sign_in: string; url: string } | Required<NeedsClient>>('POST', '/api/tools/sign-in', tool),
  cancelToolSignIn: (signIn: string) => call<unknown>('DELETE', `/api/tools/sign-in/${signIn}`),
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
