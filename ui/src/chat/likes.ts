import type { Message, PlanSummary, Task } from '../api';
import type { ChatState } from './state';

/** The person as a giver of a like: the service's `person` (0057). */
export const PERSON = 'person';

/**
 * The person's like on a message (0057): the message it goes on, the waiting questions its bubble holds (the like
 * answers each with its pick), and whether it is given. A message that offers no like and has none of the person's
 * has none.
 */
export interface Like {
  message: string;
  questions: number[];
  mine: boolean;
}

const mine = (state: ChatState, message: string) =>
  state.reactions.some((r) => r.message === message && r.agent === PERSON);

/** A question the asker recommended a choice for: its first, which the window names "<asker>'s pick". */
const picked = (t: Task) => t.form === 'choice' && t.giver !== null && !!t.choices?.length;

/**
 * The like an agent's block offers: on its shown words, or, with none, on its last step when it holds a waiting
 * question. A bubble holding a waiting question with no pick offers none: a like there would read as an answer. One
 * still streaming offers none yet.
 */
export function blockLike(
  state: ChatState,
  msgs: Message[],
  asked: number[],
  streaming: boolean,
): Like | undefined {
  if (streaming) return undefined;
  const waiting = state.tasks.filter((t) => asked.includes(t.number) && t.state === 'ready');
  const last = msgs.at(-1);
  const message = last?.kind === 'text' || waiting.length ? last?.id : undefined;
  if (!message) return undefined;
  const given = mine(state, message);
  const answers = waiting.every(picked);
  if (!answers && !given) return undefined;
  return { message, questions: answers ? waiting.map((t) => t.number) : [], mine: given };
}

/** A version of the lead's plan offers the like while it waits for the person: the like approves it. */
export function versionLike(
  state: ChatState,
  plan: PlanSummary,
  n: number,
  message: string,
): Like | undefined {
  const newest = Math.max(...plan.versions.map((v) => v.n));
  const v = plan.versions.find((x) => x.n === n);
  const waits = plan.state === 'waiting' && n === newest && !!v?.sent && !v.decided;
  const given = mine(state, message);
  return waits || given ? { message, questions: [], mine: given } : undefined;
}

/** A message between agents always offers it. */
export const saidLike = (state: ChatState, message: string): Like => ({
  message,
  questions: [],
  mine: mine(state, message),
});
