import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Failure, Message, Named, PickedName, Role, Task, ThinkingBody, ToolBody, Turn } from '../api';
import type { Out } from '../components/Outcome';
import { agreedCheck, planLine, versionOf } from '../plans/plans';
import { type AskSize, askSizes, changed, fixed } from './asks';
import { type Ending, ending } from './ends';
import { fold, type Given, type Work } from './fold';
import { blockLike, type Like, PERSON, versionLike } from './likes';
import { place } from './order';
import { heardTurns, questions } from './questions';
import { prThread, underPrs } from './reviews';
import type { Approval, ChatState } from './state';
import { type Team, team } from './team';
import { type Said, talks, threadItem } from './threads';

/** A request that failed: what was sent (none for Stop or Pause), for Retry, and why. */
export interface SendFailure {
  id: string;
  text: string;
  /** The agent it was for, so a resend goes to the same one. */
  agent?: string;
  /** The names picked in it, sent again with it. */
  picks?: PickedName[];
  failure: Failure;
}

/** One of Gathr's notes, as a line: "Vale completed: Review PR #138". */
export interface EventLine {
  kind: 'task' | 'note' | 'version' | 'changed';
  /** Who did what, ahead of the text: "Vale completed:". */
  who?: string;
  text: string;
  /** What a click on the line adds: the reason, the question. */
  more?: string;
  /** A plan line's version: it opens the plan's tab there (0062). */
  plan?: { id: number; version: number };
}

/** Gathr's own part of a message, read by the thread's message components. */
export type Custom = {
  agent?: string;
  /** The agent's id: its face opens its panel, and the panel lists its blocks. */
  by?: string;
  role?: Role;
  /** When the reply began, shown in a team chat. */
  time?: string;
  /** The speaker changed from the block before, so this one names it. */
  head?: boolean;
  /** The task a member's turn works on, after its name: "on Review PR #138". */
  on?: string;
  /** That task is done, and this is its last message: a tick after it. */
  done?: boolean;
  /** That task was cancelled or failed, and this is its last message: the word after it. */
  out?: Out;
  /** What the turn did before its message, as one row. */
  work?: Work;
  /** It has words to show, saved or streaming. */
  text?: boolean;
  /** How its turn ended, on its last block: a word on its name line, what the person can do under the bubble. */
  ending?: Ending;
  approvals?: Approval[];
  /** A message that never reached the service. */
  unsent?: SendFailure;
  /** Messages between agents, one for each text of the block, as a thread (0072). */
  thread?: Said[];
  /** The thread sits under the block before it, which it belongs to; else it stands where it began. */
  under?: boolean;
  /** Everything in the thread is settled: it starts folded, and folds when it becomes so. */
  settled?: boolean;
  /** The ask's size, on the person's message that started it (0061). */
  size?: AskSize;
  /** The person's message: the names they picked from the "@" list, drawn blue. */
  picks?: PickedName[];
  /** Who gave its text a thumbs-up: "You" first, then agents in the order given. */
  agreed?: string[];
  /** The person's like it offers, or holds (0057). */
  like?: Like;
  events?: EventLine[];
  /** A version of the lead's plan: its chip opens the plan's tab (0062). */
  plan?: { plan: number; version: number; parts: number };
  /** The person's questions asked in it, by task, each answered in place (0073). */
  asked?: number[];
  /** Gathr says it, as a speaker of its own (0073). */
  gathr?: boolean;
  /**
   * When it was said, on a block with a tick on the conversation map (0039): the person's message, or
   * the lead's reply to them, which is the last block of each of its main-line turns once it has ended.
   */
  tick?: string;
};

/** The chat in order, and where a saved message shows in it, for a search hit. */
export interface Timeline {
  items: ThreadMessageLike[];
  find: (message: string) => Spot | undefined;
}

/** A thread message, and a hit's own text in it. */
export interface Spot {
  id: string;
  text?: string;
  /** Which of the block's texts equal to `text` it is: a reply can say the same thing twice. */
  nth?: number;
}

export type Cache = Map<string, { sig: string; value: ThreadMessageLike }>;
export interface Group {
  key: string;
  turn: string | null;
  msgs: Message[];
  kind: 'person' | 'gathr' | 'reply' | 'to' | 'version' | 'question' | 'said';
  /** Where its first message is among the chat's saved ones; one with none yet sits after its turn's latest. */
  at: number;
  /** It opens its turn: the message that started it, or its first reply. */
  bound: boolean;
  /** The turn's last reply: the one its streaming text, notice and approvals go on. */
  last?: boolean;
  /** A message to an agent: the block its turn was writing when it was sent. */
  root?: Group;
  /** A question that is a block of its own, or the person's answer or Discuss to it, which has no message. */
  task?: number;
  text?: string;
  time?: string;
}

const ended = new Set(['stopped', 'waiting_sign_in', 'failed']);
const engineWait = (t: Turn) => t.held === 'claude' || t.held === 'codex';
const hasEnding = (t: Turn) => ended.has(t.status);

/**
 * Gathr's chat as assistant-ui messages, one timeline (0037). A turn is one block where it began: what it
 * did folds into a row, its last text is the message, and a later reply of the turn lands in that block,
 * so turns streaming at once never interleave. Only a person's message sent into a running turn splits it,
 * one block before and one after; the last holds the streaming text, how the turn ended and the approvals.
 * Messages between agents are a thread under the block they were sent from, a reply joining the thread of the
 * message it answers (threads.ts, 0072); Gathr's copy of one in the receiver's turn isn't shown. What Gathr told an
 * agent is drawn as Gathr's message, all but the brief of a member's own task and what a task's end told: how a task
 * ended sits on its member's last block (0051).
 *
 * A block keeps one id from its first streamed text to its saved messages, so nothing remounts when a
 * stream ends. One that didn't change is the same object as on the draw before (`cache`), so the thread
 * can tell which blocks to convert and draw again.
 */
export function toThread(state: ChatState, agents: Named[], unsent: SendFailure[], cache: Cache): Timeline {
  const crew = team(state, new Map(agents.map((a) => [a.id, a.name])));
  const saved = grouped(state);
  const { extra, hosts } = questions(state, saved);
  const groups = place(extra.length ? [...saved, ...extra] : saved, state);
  const heardIn = heardTurns(state.tasks);
  const gave = givenBy(state, crew);
  const sizes = askSizes(state, crew);
  const lastOf = lastBlocks(groups, state);
  const items: ThreadMessageLike[] = [];
  /** Each group's block; one not drawn has none. */
  const shown = new Map<Group, { id: string; msgs: Message[] }>();
  const add = (value: ThreadMessageLike, from: Group[]) => {
    const msgs = from.flatMap((g) => g.msgs);
    for (const g of from) shown.set(g, { id: value.id as string, msgs });
    items.push(value);
  };
  let speaker: string | undefined;
  const { of, quiet, spoken } = talks(groups, state, crew, hasEnding);
  const agreed = (message: string) => agreedBy(state, crew, message);
  const thread = (talk: NonNullable<ReturnType<typeof of.get>>) => {
    const item = threadItem(cache, talk, state, crew, { quiet, spoken, gave, agreedBy: agreed });
    add(item, [
      ...talk.groups,
      ...[...quiet].flatMap(([work, to]) => (talk.groups.includes(to) ? work : [])),
    ]);
    speaker = undefined;
  };
  const under = new Map([...new Set(of.values())].flatMap((t) => (t.root ? [[t.root, t] as const] : [])));
  const onPr = underPrs(groups, state, (g) => !quiet.has(g));
  const numbered = new Map(state.tasks.map((t) => [t.number, t]));
  const prWork = new Map<Group, Group[]>();
  for (const [g, root] of onPr) prWork.set(root, [...(prWork.get(root) ?? []), g]);
  /** Who a reply block is by and the task it is on: a block that follows its speaker's isn't named again. */
  const heading = (g: Group) => {
    const turn = g.turn ? state.turns[g.turn] : undefined;
    // Only a member or the reviewer is headed by its task; the lead always by its role.
    const led = !crew.role(turn?.agent) || crew.role(turn?.agent) === 'supervisor';
    const on = led || turn?.task == null ? undefined : numbered.get(turn.task)?.what;
    const last = turn?.task != null && lastOf.get(turn.task) === g;
    return { on, last, who: `${turn?.agent}|${on}` };
  };
  const reply = (g: Group, head: boolean) =>
    block(cache, g, state, crew, gave, { ...heading(g), head, asked: hosts.get(g) });
  for (const g of groups) {
    if (onPr.has(g)) continue;
    if (g.kind === 'to') {
      const talk = of.get(g);
      if (talk && !talk.root && talk.groups[0] === g) thread(talk);
      continue;
    }
    if (quiet.has(g)) continue;
    if (g.kind === 'gathr') {
      const lines = heardIn.has(g.turn ?? '') ? [] : heard(g, state, crew);
      if (!lines.length) continue;
      add(eventsOf(cache, g.key, lines, g.msgs[0].time), [g]);
      speaker = undefined;
      continue;
    }
    if (g.kind === 'question' || g.kind === 'said') {
      add(g.kind === 'said' ? saidBlock(cache, g) : questionBlock(cache, g, state, crew), [g]);
      speaker = undefined;
      continue;
    }
    if (g.kind === 'version') {
      // Named as the lead's reply blocks are: the lead's turn works on no task.
      const who = `${state.turns[g.turn ?? '']?.agent}|${undefined}`;
      add(versionBlock(cache, g, state, crew, who !== speaker), [g]);
      speaker = who;
      continue;
    }
    if (g.kind === 'person') {
      const m = g.msgs[0];
      const size = sizes.get(m.id);
      add(
        kept(cache, g.key, `${m.body}|${JSON.stringify([size, m.picks])}`, () => toUser(m, size)),
        [g],
      );
      speaker = undefined;
      continue;
    }
    const { who } = heading(g);
    add(reply(g, who !== speaker), [g]);
    speaker = who;
    const talk = under.get(g);
    if (talk) thread(talk);
    const work = prWork.get(g);
    if (work) {
      // Each block of the PR's work, then the talk it began, as one thread.
      const parts = work.flatMap((w) => {
        const t = under.get(w);
        const talked = t
          ? [threadItem(cache, t, state, crew, { quiet, spoken, gave, agreedBy: agreed })]
          : [];
        const from = t
          ? [...t.groups, ...[...quiet].flatMap(([q, to]) => (t.groups.includes(to) ? q : []))]
          : [];
        return [{ item: reply(w, true), from: [w] }, ...talked.map((item) => ({ item, from }))];
      });
      const settled = work.every((w) => settledTask(state, numbered, w));
      add(
        prThread(
          cache,
          g,
          parts.map((p) => p.item),
          settled,
        ),
        parts.flatMap((p) => p.from),
      );
      speaker = undefined;
    }
  }
  // A send that opened a turn fails that turn too, and the turn's notice already says so.
  const told = new Set(Object.values(state.turns).map((t) => t.ref));
  for (const f of unsent) {
    if (f.failure.ref && told.has(f.failure.ref)) continue;
    items.push(
      kept(cache, `f-${f.id}`, `${f.text}|${f.failure.error}|${f.failure.ref}|${f.failure.why}`, () =>
        failed(f),
      ),
    );
  }

  const find = (message: string): Spot | undefined => {
    const hit = state.messages.find((x) => x.id === message);
    // Gathr's copy of an agent's message shows as the message it carries.
    const m = (hit?.copy_of && state.messages.find((x) => x.id === hit.copy_of)) || hit;
    if (!m) return undefined;
    const own = groups.find((g) => g.msgs.includes(m));
    const at = own && shown.get(own);
    if (at && m.kind === 'text' && own.kind !== 'gathr') {
      const words = (x: Message) => spoken.get(x.id)?.text ?? x.body;
      const before = at.msgs.slice(0, at.msgs.indexOf(m));
      const nth = before.filter((x) => x.kind === 'text' && words(x) === words(m)).length;
      return { id: at.id, text: words(m) as string, nth };
    }
    if (at) return { id: at.id };
    // A task's brief isn't drawn: its turn's block stands for it, or the giver's until that turn starts.
    const task = state.turns[m.turn ?? '']?.task;
    const gave = task == null ? undefined : state.tasks.find((t) => t.number === task)?.given_in;
    const blocks = groups.filter((g) => g.kind === 'reply' && shown.has(g));
    const stand = [m.turn, gave].flatMap((turn) => blocks.find((g) => !!turn && g.turn === turn) ?? []);
    return stand.length ? { id: shown.get(stand[0])?.id as string } : undefined;
  };
  return { items, find };
}

function grouped(state: ChatState): Group[] {
  // The nth reply of a turn is `a-<turn>`, then `a-<turn>-<n>`: history only grows at its end, so n is stable.
  const counts = new Map<string, number>();
  const reply = (turn: string, at: number): Group => {
    const n = counts.get(turn) ?? 0;
    counts.set(turn, n + 1);
    return { key: n ? `a-${turn}-${n}` : `a-${turn}`, turn, msgs: [], kind: 'reply', at, bound: n === 0 };
  };
  const groups: Group[] = [];
  // Each turn's reply that its next message joins, until a person's message sent into the turn splits it.
  const open = new Map<string, Group>();
  // Each turn's latest saved message, of any kind.
  const latest = new Map<string, number>();
  const versions = new Set(state.plans.flatMap((p) => p.versions.map((v) => v.message)));
  state.messages.forEach((m, at) => {
    const bound = !!m.turn && !latest.has(m.turn);
    if (m.turn) latest.set(m.turn, at);
    const one = { turn: m.turn, msgs: [m], at };
    if (m.role === 'user') {
      groups.push({ ...one, key: m.id, kind: 'person', bound });
      if (m.turn) open.delete(m.turn);
    } else if (m.role === 'gathr') {
      if (!m.copy_of) groups.push({ ...one, key: m.id, kind: 'gathr', bound });
    } else if (versions.has(m.id)) {
      // The lead's text after the plan call starts a new block after the chip, never above it.
      groups.push({ ...one, key: m.id, kind: 'version', bound });
      if (m.turn) open.delete(m.turn);
    } else if (m.to) {
      groups.push({ ...one, key: m.id, kind: 'to', bound: false, root: open.get(m.turn ?? '') });
    } else if (m.turn && open.has(m.turn)) open.get(m.turn)?.msgs.push(m);
    else if (m.turn) {
      const g = { ...reply(m.turn, at), msgs: [m] };
      groups.push(g);
      open.set(m.turn, g);
    } else groups.push({ ...one, key: `a-${m.id}`, kind: 'reply', bound: false });
  });
  // A turn with streaming text, a notice or an approval but no reply since its latest message still
  // has one, right after that message.
  const waiting = new Set(state.approvals.map((a) => a.turn));
  for (const t of Object.values(state.turns)) {
    const last = open.get(t.id) ?? groups.findLast((g) => g.turn === t.id && g.kind !== 'to');
    if (last?.kind === 'reply') {
      last.last = true;
      continue;
    }
    if (!(state.partial[t.id] || hasEnding(t) || waiting.has(t.id))) continue;
    const after = latest.get(t.id);
    groups.push({ ...reply(t.id, after === undefined ? Number.POSITIVE_INFINITY : after + 0.5), last: true });
  }
  return groups;
}

/** Each task's last block of a turn on it: how the task ended is said there, once (0051). */
function lastBlocks(groups: Group[], state: ChatState): Map<number, Group> {
  const last = new Map<number, Group>();
  for (const g of groups) {
    const task = g.kind === 'reply' ? state.turns[g.turn ?? '']?.task : undefined;
    if (task != null) last.set(task, g);
  }
  return last;
}

/** The task each give_task call made: a turn's calls that worked and the tasks it gave, in order. */
function givenBy(state: ChatState, crew: Team) {
  const calls = new Map<string, Message[]>();
  for (const m of state.messages) {
    const body = m.kind === 'tool' ? (m.body as ToolBody) : undefined;
    if (!m.turn || body?.tool !== 'mcp__gathr__give_task' || body.status !== 'ok') continue;
    calls.set(m.turn, [...(calls.get(m.turn) ?? []), m]);
  }
  const gave = new Map<string, Given>();
  for (const [turn, made] of calls) {
    const tasks = state.tasks
      // A plan's check and its parts' tasks are Gathr's gives, with no call of the turn's.
      .filter((t) => t.given_in === turn && t.kind !== 'person' && !t.checks && t.plan == null)
      .sort((a, b) => a.number - b.number);
    // Only when they pair off: a call for each task, a task for each call.
    if (tasks.length !== made.length) continue;
    for (const [i, m] of made.entries()) gave.set(m.id, { task: tasks[i], owner: crew.name(tasks[i].owner) });
  }
  return gave;
}

function block(
  cache: Cache,
  group: Group,
  state: ChatState,
  crew: Team,
  gave: Map<string, Given>,
  { on, last: lastOfTask, head, asked }: { on?: string; last: boolean; head: boolean; asked?: number[] },
) {
  const id = group.turn;
  const turn = id ? state.turns[id] : undefined;
  const partial = group.last && id ? (state.partial[id] ?? '') : '';
  const approvals = group.last ? state.approvals.filter((a) => a.turn === id) : [];
  const agent = crew.name(turn?.agent);
  const role = crew.role(turn?.agent);
  const parts = group.msgs.map((m) => {
    if (m.kind === 'text') return m.id;
    const { status, duration_ms } = m.body as ToolBody | ThinkingBody;
    const made = gave.get(m.id);
    return `${m.id}:${status}:${duration_ms}:${made?.task.number}:${made?.task.what}:${made?.owner}`;
  });
  // The turn that answers a stopped one is its carry-on: once it runs or has ended, the work went on
  // there. One still waiting, or stopped (it may never have started), leaves Continue with the person.
  const went = ['running', 'done', 'failed'];
  const carried =
    turn?.status === 'stopped' &&
    Object.values(state.turns).some((t) => t.answers === id && went.includes(t.status));
  const task = state.tasks.find((t) => t.number === turn?.task);
  const end = group.last && turn ? ending(turn, task, carried) : undefined;
  const ended = end ? `${turn?.status}|${turn?.ref}|${end.word}|${end.act}` : '';
  // The lead: the Supervisor in a team chat, whoever answers in a chat with one agent.
  const lead = state.members.length > 1 ? role === 'supervisor' : true;
  // A turn still running has no last words yet, so a streamed token never touches the map.
  // One waiting for sign-in runs again later under the same id.
  const over = !['queued', 'waiting_sign_in', 'running'].includes(turn?.status ?? '');
  const toPerson = group.last && !!id && lead && over && !crew.rootOf(id);
  const tick = toPerson
    ? (group.msgs.findLast((m) => m.kind === 'text')?.time ?? turn?.created ?? '')
    : undefined;
  // Only the shown text's: an earlier text folds into the row, and its thumbs-up would sit under words
  // the giver didn't agree with.
  const last = group.msgs.at(-1);
  const like = blockLike(state, group.msgs, asked ?? [], !!partial);
  const agreed = !partial && last && (last.kind === 'text' || like) ? agreedBy(state, crew, last.id) : [];
  // A blocked, stopped or held task is its owner's face's mark, never a tick (0071); a word of the turn's says enough.
  const shown = on && lastOfTask ? task?.state : undefined;
  const done = shown === 'done';
  const out = !end?.word && (shown === 'cancelled' || shown === 'failed') ? shown : undefined;
  const who = `${agent}|${role}|${on}|${done}|${out}|${head}|${tick}|${agreed}|${JSON.stringify(like)}`;
  const answering = approvals.map((a) => `${a.request}:${a.answer?.allow}:${a.answer?.failure?.ref}`);
  const sig = `${parts}|${partial.length}|${ended}|${answering}|${who}|${asked}`;
  const hit = cache.get(group.key);
  if (hit?.sig === sig) return hit.value;
  const time = group.msgs[0]?.time ?? turn?.created;
  const custom: Custom = {
    agent,
    by: turn?.agent ?? undefined,
    role,
    time,
    on,
    done,
    out,
    head,
    approvals,
    tick,
    agreed,
    asked,
    like,
  };
  if (end) custom.ending = end;
  const value = toReply(group, partial, group.last ? turn : undefined, custom, gave);
  cache.set(group.key, { sig, value });
  return value;
}

function toReply(
  group: Group,
  partial: string,
  turn: Turn | undefined,
  custom: Custom,
  gave: Map<string, Given>,
): ThreadMessageLike {
  const status = turn?.status;
  const began = group.bound ? turn?.created : undefined;
  const { work, text } = fold(group.msgs, partial, status === 'running', gave, began);
  if (work) custom.work = work;
  if (text) custom.text = true;
  return {
    id: group.key,
    role: 'assistant',
    content: text ? [{ type: 'text', text }] : [],
    status:
      status === 'running' || (status === 'queued' && !(turn && engineWait(turn)))
        ? { type: 'running' }
        : status === 'failed'
          ? { type: 'incomplete', reason: 'error' }
          : status === 'stopped'
            ? { type: 'incomplete', reason: 'cancelled' }
            : { type: 'complete', reason: 'stop' },
    metadata: { custom },
  };
}

/** Who gave a message a thumbs-up, by name: "You" first, then agents in the order given; one Gathr can't name is left out. */
function agreedBy(state: ChatState, crew: Team, message: string): string[] {
  const on = state.reactions.filter((r) => r.message === message);
  const agents = on.flatMap((r) => (r.agent !== PERSON && crew.name(r.agent)) || []);
  return on.some((r) => r.agent === PERSON) ? ['You', ...agents] : agents;
}

/** A version of the lead's plan: the speaker, its chip and who gave it a thumbs-up; its words are the tab's. */
function versionBlock(
  cache: Cache,
  group: Group,
  state: ChatState,
  crew: Team,
  head: boolean,
): ThreadMessageLike {
  const m = group.msgs[0];
  const found = versionOf(state.plans, m.id);
  const agent = state.turns[group.turn ?? '']?.agent;
  const custom: Custom = {
    agent: crew.name(agent),
    by: agent ?? undefined,
    role: crew.role(agent),
    time: m.time,
    head,
    agreed: [
      ...new Set([...agreedBy(state, crew, m.id), ...(found ? agreedCheck(found.version, state, crew) : [])]),
    ],
    plan: found && { plan: found.plan.id, version: found.version.n, parts: found.version.parts },
    like: found && versionLike(state, found.plan, found.version.n, m.id),
  };
  return kept(cache, group.key, JSON.stringify(custom), () => ({
    id: group.key,
    role: 'assistant',
    content: [],
    metadata: { custom },
  }));
}

// The service's words that open a turn after the person changes a size (plan_wait.rs `told`): for the agent, never
// drawn. Gathr's own short line says what the change held or released (0073).
const RESIZED = 'The person made "';

// What a task's end told (tasks/words.rs): a hear-back's first line, and Gathr's line for a task that ended done (`DONE`,
// 0051). The agents read both; the person reads each end on its member's own last message.
const TOLD =
  /^(?:The (?:task|\d+ tasks) you gave (?:is|are) settled|Where the (?:task|\d+ tasks) you gave stands?|A task is done):\n/;

/**
 * What Gathr told an agent, as lines in its own words. None for the brief of the turn's own task (the member's block
 * is headed by the task), for what a size change tells the lead (0073), or for what a task's end told (`TOLD`).
 */
function heard(group: Group, state: ChatState, crew: Team): EventLine[] {
  const m = group.msgs[0];
  if (state.caught.includes(m.id)) return [changed(m.body as string)];
  const plan = planLine(m, state, crew);
  if (plan !== undefined) return plan ? [plan] : [];
  const text = m.body as string;
  const turn = state.turns[m.turn ?? ''];
  if (turn && text.startsWith(RESIZED)) return [];
  const fix = m.turn ? undefined : fixed(text);
  if (fix) return [fix];
  if (TOLD.test(text)) return [];
  // The service's brief (tasks/words.rs `brief`) opens with the task's number and its giver.
  const brief = turn?.task != null && text.startsWith(`Task #${turn.task} from `);
  return brief ? [] : [{ kind: 'note', text }];
}

/**
 * A question with no block of its asker's to sit in: Gathr's own, as Gathr's message, or an agent's whose turn drew
 * none, as that agent's. Its words are the block's (Asked.tsx); this is who asks and when.
 */
function questionBlock(cache: Cache, group: Group, state: ChatState, crew: Team): ThreadMessageLike {
  const t = state.tasks.find((x) => x.number === group.task);
  const time = t && new Date(t.created_us / 1000).toISOString();
  const custom: Custom = t?.giver
    ? {
        agent: crew.name(t.giver),
        by: t.giver,
        role: crew.role(t.giver),
        time,
        head: true,
        asked: [t.number],
      }
    : { gathr: true, time, asked: [group.task as number] };
  return kept(cache, group.key, JSON.stringify(custom), () => ({
    id: group.key,
    role: t?.giver ? 'assistant' : 'system',
    content: t?.giver ? [] : [{ type: 'text', text: t?.what ?? '' }],
    metadata: { custom },
  }));
}

/** The person's answer or Discuss to a question, as their own message, when they sent it. */
function saidBlock(cache: Cache, group: Group): ThreadMessageLike {
  const text = group.text ?? '';
  return kept(cache, group.key, `${text}|${group.time}`, () => ({
    id: group.key,
    role: 'user',
    content: [{ type: 'text', text }],
    metadata: { custom: { time: group.time } satisfies Custom },
  }));
}

/** The block made on an earlier draw, while what it's made from (`sig`) is the same. */
function kept(cache: Cache, key: string, sig: string, make: () => ThreadMessageLike): ThreadMessageLike {
  const hit = cache.get(key);
  if (hit?.sig === sig) return hit.value;
  const value = make();
  cache.set(key, { sig, value });
  return value;
}

/** Gathr's lines, said by Gathr at its message's `time`. */
function eventsOf(cache: Cache, key: string, events: EventLine[], time: string): ThreadMessageLike {
  const sig = `${time}|${JSON.stringify(events)}`;
  const hit = cache.get(key);
  if (hit?.sig === sig) return hit.value;
  const text = events.map((e) => [e.who, e.text].filter(Boolean).join(' ')).join('\n');
  const custom: Custom = { events, gathr: true, time };
  const value: ThreadMessageLike = {
    id: key,
    role: 'system',
    content: [{ type: 'text', text }],
    metadata: { custom },
  };
  cache.set(key, { sig, value });
  return value;
}

function toUser(m: Message, size?: AskSize): ThreadMessageLike {
  return {
    id: m.id,
    role: 'user',
    content: [{ type: 'text', text: m.body as string }],
    metadata: { custom: { tick: m.time, size, picks: m.picks ?? undefined } satisfies Custom },
  };
}

/** A block's task is done or cancelled, and its turn has ended. */
function settledTask(state: ChatState, numbered: Map<number, Task>, g: Group): boolean {
  const turn = state.turns[g.turn ?? ''];
  const task = numbered.get(turn?.task ?? -1);
  const over = !['queued', 'waiting_sign_in', 'running'].includes(turn?.status ?? '');
  return over && (task?.state === 'done' || task?.state === 'cancelled');
}

function failed(f: SendFailure): ThreadMessageLike {
  const custom: Custom = { unsent: f, picks: f.picks };
  return { id: `f-${f.id}`, role: 'user', content: [{ type: 'text', text: f.text }], metadata: { custom } };
}
