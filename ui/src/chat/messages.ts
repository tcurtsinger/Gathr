import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Failure, Named, PickedName, Role, Task } from '../api';
import type { Out } from '../components/Outcome';
import type { Said as State } from '../tasks/rows';
import type { AskSize } from './asks';
import type { Ending } from './ends';
import type { Work } from './fold';
import type { Like } from './likes';
import type { Approval, ChatState } from './state';
import type { Said } from './threads';
import { drawn } from './timeline';

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

/** A main message's state at its name line's right (0075): its word, its PR, and where a Done of an ask leads. */
export interface MainState {
  said: State;
  pr?: number;
  /** The topic of the ask's newest result: its Done scrolls there. */
  link?: string;
}

/** Where a block sits in a team chat (0075): which main message it is about, and whether it is that one. */
export type Place = {
  topic?: string;
  /** In its main message's thread: smaller, inside the thread's rule. */
  reply?: boolean;
  state?: MainState;
  /** An ask's result: the ask it closes and its first line, quoted. */
  result?: { ask: string; quote: string };
};

/**
 * The row under a main message: its thread's fold line (how many replies, and whether it is open), and the main
 * message's thumbs-up on the same row. With no replies it holds the thumbs-up alone.
 */
export interface Fold {
  topic: string;
  open: boolean;
  count: number;
  agreed?: string[];
  like?: Like;
}

/** A task the lead hands out, as its main message: from the task's record, as the Tasks view reads it. */
export interface Brief {
  number: number;
  owner?: string;
  what: string;
  why?: string;
  done?: string;
  proof?: string;
  where?: string;
  size?: string;
  lines?: number;
}

/** Gathr's own part of a message, read by the thread's message components. */
export type Custom = Place & {
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
  /** A message between agents (0072). */
  said?: Said;
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
  /** A task handed out, as its main message. */
  brief?: Brief;
  /** A thread's fold line. */
  fold?: Fold;
  /** "Show N earlier replies" in a long thread. */
  earlier?: { topic: string; n: number };
  /**
   * When it was said, on a block with a tick on the conversation map (0039): the person's message and an ask's result;
   * in a chat of one, the lead's reply to the person.
   */
  tick?: string;
};

/** The chat in order, and where a saved message shows in it, for a search hit. */
export interface Timeline {
  /** What is drawn: main messages, fold lines and the replies of open threads. */
  items: ThreadMessageLike[];
  /** Every block, folded or not, in the same order: what the agent's panel, the marks and the status line read. */
  all: ThreadMessageLike[];
  find: (message: string) => Spot | undefined;
  /** The threads whose main message is handled: those start folded when the chat opens. */
  handled: Set<string>;
}

/** A thread message, and a hit's own text in it; `topic` when it is in a folded thread or behind its earlier replies. */
export interface Spot {
  id: string;
  text?: string;
  /** Which of the block's texts equal to `text` it is: a reply can say the same thing twice. */
  nth?: number;
  topic?: string;
}

/** Which threads are open: the person's own folds while the chat is open, else folded if handled when it opened. */
export interface View {
  open: Map<string, boolean>;
  /** Handled when the chat opened; none yet: handled now. */
  start?: Set<string>;
  /** Long threads whose earlier replies the person showed. */
  earlier: { has: (topic: string) => boolean };
}

export type Cache = Map<string, { sig: string; value: ThreadMessageLike }>;

/** A task's record as its main message reads it. */
export const briefOf = (t: Task, owner?: string): Brief => ({
  number: t.number,
  owner,
  what: t.what,
  why: t.why ?? undefined,
  done: t.done,
  proof: t.proof,
  where: t.where,
  size: t.size,
  lines: t.lines,
});

/**
 * Gathr's chat as assistant-ui messages. A chat of one agent reads as one timeline (0037); a team chat as main
 * messages in time order, each with its thread (0075), from the moment it became a team.
 */
export function toThread(
  state: ChatState,
  agents: Named[],
  unsent: SendFailure[],
  cache: Cache,
  view?: View,
): Timeline {
  return drawn(state, new Map(agents.map((a) => [a.id, a.name])), unsent, cache, view);
}
