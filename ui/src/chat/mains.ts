import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Task } from '../api';
import { mainSaid, openSaid, type Said as State, versionSaid } from '../tasks/rows';
import type { Group } from './groups';
import type { Answer, Custom, Fold, MainState } from './messages';
import type { ChatState } from './state';

/** A thread opened long shows this many of its newest replies under "Show N earlier replies". */
export const NEWEST = 3;

/** One main message and the blocks in its thread, in the chat's order. */
export interface Thread {
  topic: string;
  /** Its own block; none for a task, drawn from its record. */
  main?: Group;
  task?: Task;
  replies: Group[];
  /** When it was put to someone, in ms: the main line reads in this order. */
  at: number;
}

const OPEN = new Set(['queued', 'running', 'waiting_sign_in']);

/**
 * The chat as main messages, each with its thread (0075), in time order. A main message is the person's message, a
 * task the lead hands out, a plan version, a question to the person, an ask's result, a line of Gathr's that is a main
 * message, or a turn nothing ties; every other block sits in its topic's thread, in the chat's order.
 */
export function threadsOf(groups: Group[], topics: Map<Group, string>, state: ChatState): Thread[] {
  const tasks = new Map(state.tasks.map((t) => [t.number, t]));
  const byTopic = new Map<string, Thread>();
  const thread = (topic: string) => {
    let t = byTopic.get(topic);
    if (!t) {
      t = { topic, replies: [], at: Number.POSITIVE_INFINITY };
      byTopic.set(topic, t);
    }
    return t;
  };
  const asked = new Set(state.tasks.flatMap((t) => (t.kind === 'person' ? [`t:${t.number}`] : [])));
  for (const g of groups) {
    const topic = topics.get(g);
    if (!topic) continue;
    const t = thread(topic);
    if (!t.main && isMain(g, topic, asked.has(topic))) t.main = g;
    else t.replies.push(g);
  }
  for (const t of byTopic.values()) {
    const n = t.topic.startsWith('t:') ? Number(t.topic.slice(2)) : Number.NaN;
    const task = tasks.get(n);
    if (!t.main && task && task.kind !== 'person') t.task = task;
    // A thread whose main message isn't drawn (a link the service couldn't follow) opens with its first block.
    if (!t.main && !t.task) t.main = t.replies.shift();
    t.at = t.task ? t.task.created_us / 1000 : timeOf(t.main, state);
  }
  return [...byTopic.values()].sort((a, b) => a.at - b.at);
}

function isMain(g: Group, topic: string, question: boolean): boolean {
  if (topic.startsWith('m:')) return g.msgs[0]?.id === topic.slice(2);
  // A question's main message is the block that asked it; a task's is drawn from its record.
  if (topic.startsWith('t:')) return question && (g.kind === 'question' || g.kind === 'reply');
  // A result is the lead's own words, never Gathr's that opened its turn; a turn nothing ties, its first block that
  // draws anything (timeline.ts).
  return !topic.startsWith('r:') || g.kind !== 'gathr';
}

/** When a block was said, in ms; a question or answer without a message, when it was asked or sent. */
export function timeOf(g: Group | undefined, state: ChatState): number {
  if (!g) return Number.POSITIVE_INFINITY;
  const time = g.msgs[0]?.time ?? g.time ?? state.turns[g.turn ?? '']?.created;
  if (time) return Date.parse(time);
  const task = state.tasks.find((t) => t.number === g.task);
  return task ? task.created_us / 1000 : Number.POSITIVE_INFINITY;
}

/** A thread's turns are all over, and none waits on an approval. */
function quiet(t: Thread, state: ChatState): boolean {
  const turns = new Set([t.main, ...t.replies].flatMap((g) => (g?.turn ? [g.turn] : [])));
  if (state.approvals.some((a) => turns.has(a.turn))) return false;
  return ![...turns].some((id) => OPEN.has(state.turns[id]?.status ?? ''));
}

/** A main message's state word and whether its thread is handled: what starts folded when the chat opens. */
export function stateOf(
  t: Thread,
  state: ChatState,
  results: Map<string, string>,
): { state?: MainState; handled: boolean } {
  const still = quiet(t, state);
  if (t.task) {
    const said = mainSaid(t.task, state.tasks);
    return { state: { said, pr: t.task.pr ?? undefined }, handled: still && ended(said) };
  }
  const g = t.main;
  if (g?.kind === 'question' || (g?.kind === 'reply' && t.topic.startsWith('t:'))) {
    const task = state.tasks.find((x) => `t:${x.number}` === t.topic);
    const said = task && mainSaid(task, state.tasks);
    return { state: said && { said }, handled: still && !!said && ended(said) };
  }
  if (g?.kind === 'version') {
    for (const plan of state.plans) {
      const version = plan.versions.find((v) => v.message === g.msgs[0].id);
      if (!version) continue;
      const said = versionSaid(plan, version);
      return { state: said && { said }, handled: still && version.decided != null };
    }
  }
  if (g?.kind === 'person') {
    const id = g.msgs[0].id;
    const ask = state.asks.find((a) => a.id === id);
    const open = ask?.open ?? !still;
    return {
      state: { said: openSaid(open), link: open ? undefined : results.get(id) },
      handled: !open && still,
    };
  }
  return { handled: still };
}

const ended = (s: State) =>
  'done' in s || ('icon' in s && s.icon !== 'merge') || ('word' in s && s.word !== 'Checking');

/** The ask a result closes: what the turn it answers was about, or the ask of the tasks that turn gave. */
export function askOf(
  turn: string,
  state: ChatState,
  topics: Map<Group, string>,
  groups: Group[],
): string | undefined {
  const answers = state.turns[turn]?.answers;
  if (!answers) return undefined;
  const asks = new Set(state.asks.map((a) => a.id));
  const about = state.turns[answers]?.topic ?? groups.find((g) => g.turn === answers && topics.has(g))?.key;
  if (about?.startsWith('m:') && asks.has(about.slice(2))) return about.slice(2);
  return state.tasks.find((t) => t.given_in === answers && t.ask)?.ask;
}

/**
 * The row under a main message: how many replies its thread has and whether it is open, with the main message's
 * thumbs-up; none when it has neither.
 */
export function foldOf(
  topic: string,
  main: ThreadMessageLike,
  count: number,
  open: boolean,
  answer?: Answer,
): Fold | undefined {
  const own = (main.metadata?.custom ?? {}) as Custom;
  const agreed = own.agreed ?? own.said?.agreed;
  if (!count && !agreed?.length) return undefined;
  return {
    topic,
    open,
    count,
    ...(agreed?.length && { agreed, like: own.like ?? own.said?.like }),
    ...(answer && { answer }),
  };
}

/**
 * Which of an open thread's replies show: all of a short one; of a long one its newest, and any that holds an
 * approval waiting, a failed step or a running turn, behind "Show N earlier replies" for the rest.
 */
export function newest(
  replies: ThreadMessageLike[],
  earlier: boolean,
): { shown: ThreadMessageLike[]; hid: number } {
  if (earlier || replies.length <= NEWEST) return { shown: replies, hid: 0 };
  const from = replies.length - NEWEST;
  const shown = replies.filter((r, i) => i >= from || keeps(r));
  return { shown, hid: replies.length - shown.length };
}

function keeps(r: ThreadMessageLike): boolean {
  const own = (r.metadata?.custom ?? {}) as Custom;
  return (
    r.status?.type === 'running' ||
    !!own.approvals?.length ||
    !!own.work?.failed ||
    !!own.work?.running ||
    !!own.ending?.bad
  );
}
