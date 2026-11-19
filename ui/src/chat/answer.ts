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
