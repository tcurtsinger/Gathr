import type { Group } from './groups';
import type { Thread } from './mains';
import type { Answer } from './messages';
import type { ChatState } from './state';

/** The chat's lead, else its own agent: who gets a reply to a line of Gathr's, or to the person's own ask. */
function lead(state: ChatState): string | undefined {
  const supervisor = state.members.find((m) => m.role === 'supervisor')?.agent;
  return supervisor ?? Object.values(state.turns).findLast((t) => t.agent)?.agent ?? undefined;
}

/**
 * What Reply on a block answers, in the thread `topic` (0076): an agent's words go to their author, a line of Gathr's
 * to the lead. The person's own words, their answers and Gathr's own question offer no Reply.
 */
export function answerOf(g: Group, topic: string, state: ChatState): Answer | undefined {
  switch (g.kind) {
    case 'person':
    case 'said':
    case 'question':
      return undefined;
    case 'gathr': {
      const line = g.msgs[0];
      return line?.kind === 'text' ? { topic, message: line.id, agent: lead(state) } : undefined;
    }
    default: {
      const agent = g.turn ? state.turns[g.turn]?.agent : undefined;
      const words = g.msgs.findLast((m) => m.kind === 'text' && m.role === 'assistant');
      return words && agent ? { topic, message: words.id, agent } : undefined;
    }
  }
}

/**
 * What Reply on a thread's line answers, and on a task's message: the task, to the agent doing it; an agent's question,
 * to the asker (none for Gathr's own, which can't be talked over); the person's ask, to the lead; else the main
 * message, as Reply on it would.
 */
export function threadAnswer(t: Thread, main: Answer | undefined, state: ChatState): Answer | undefined {
  if (t.task) return { topic: t.topic, task: t.task.number, agent: t.task.owner ?? undefined };
  const n = t.topic.startsWith('t:') ? Number(t.topic.slice(2)) : Number.NaN;
  const question = state.tasks.find((x) => x.number === n && x.kind === 'person');
  if (question) return question.giver ? { topic: t.topic, task: n, agent: question.giver } : undefined;
  if (t.main?.kind === 'person') return { topic: t.topic, message: t.main.msgs[0].id, agent: lead(state) };
  return main;
}

/**
 * A chat drawn flat (one agent, or before it became a team): each reply of the person's, and what its turn said after
 * it, sit in a thread under the message it answers. A reply to a reply joins the same thread: threads never nest.
 */
export function flatThreads(flat: Group[]): Map<Group, Group[]> {
  const holder = new Map<string, Group>();
  for (const g of flat) for (const m of g.msgs) holder.set(m.id, g);
  const mainOf = new Map<Group, Group>();
  const threads = new Map<Group, Group[]>();
  const turns = new Map<string, Group>();
  for (const g of [...flat].sort((a, b) => a.at - b.at)) {
    const first = g.msgs[0];
    let main: Group | undefined;
    if (g.kind === 'person') {
      const target = first?.reply_to ? holder.get(first.reply_to) : undefined;
      main = target && (mainOf.get(target) ?? target);
      // What the turn says after the person's message is about that message: a reply's thread, or none.
      if (g.turn) {
        if (main) turns.set(g.turn, main);
        else turns.delete(g.turn);
      }
    } else if (g.turn) main = turns.get(g.turn);
    if (!main || main === g) continue;
    mainOf.set(g, main);
    threads.set(main, [...(threads.get(main) ?? []), g]);
  }
  return threads;
}
