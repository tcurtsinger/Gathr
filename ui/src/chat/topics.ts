import type { Group } from './groups';
import type { ChatState } from './state';
import type { Spoken } from './threads';

interface Carried {
  at: number;
  turn: string;
  of: string;
}

/**
 * Where Gathr's copy of the person's message entered a running turn (0028), in saved order: the copy isn't drawn, but
 * the turn splits there as it does for the person's own message.
 */
function carried(state: ChatState): Carried[] {
  const persons = new Set(state.messages.filter((m) => m.role === 'user').map((m) => m.id));
  const seen = new Set<string>();
  const found: Carried[] = [];
  state.messages.forEach((m, at) => {
    if (!m.turn) return;
    const first = !seen.has(m.turn);
    seen.add(m.turn);
    if (!first && m.role === 'gathr' && m.copy_of && persons.has(m.copy_of))
      found.push({ at, turn: m.turn, of: m.copy_of });
  });
  return found;
}

/**
 * The main message each block sits under (0075). The service names it for every turn, task and line said in no turn
 * (`topic`); the window keeps its two rules inside a turn: a turn splits where the person's message entered it, or
 * Gathr's copy of it (0028), and the blocks after take that message's topic; a message between agents that answers one
 * Gathr carried into its turn sits where what it answers sits. A turn the service hasn't named yet is its own main
 * message until it does.
 */
export function topicsOf(
  groups: Group[],
  state: ChatState,
  spoken: Map<string, Spoken>,
  hosts: Map<Group, number[]>,
): Map<Group, string> {
  const topics = new Map<Group, string>();
  const entered = new Map<string, string>();
  const ofMessage = new Map<string, string>();
  const ofTurn = (turn: string) => entered.get(turn) ?? state.turns[turn]?.topic ?? `b:${turn}`;
  const splits = carried(state);
  // In saved order: a split applies to the blocks saved after it, and a reply after what it answers.
  for (const g of [...groups].sort((a, b) => a.at - b.at)) {
    while (splits.length && splits[0].at < g.at) {
      const { turn, of } = splits.shift() as Carried;
      entered.set(turn, ofMessage.get(of) ?? `m:${of}`);
    }
    const topic = topicOf(g);
    topics.set(g, topic);
    for (const m of g.msgs) ofMessage.set(m.id, topic);
  }
  return topics;

  function topicOf(g: Group): string {
    const first = g.msgs[0];
    switch (g.kind) {
      case 'person': {
        // A reply sits in the thread of what it answers, and so does what its turn says after it (0076).
        const answered =
          (first.reply_to && ofMessage.get(first.reply_to)) ||
          (first.reply_task != null &&
            (state.tasks.find((t) => t.number === first.reply_task)?.topic ?? `t:${first.reply_task}`));
        if (answered) {
          if (g.turn) entered.set(g.turn, answered);
          return answered;
        }
        // A person's message into a running turn: what the turn says after it is about it.
        if (g.turn && !g.bound) entered.set(g.turn, `m:${first.id}`);
        return `m:${first.id}`;
      }
      case 'version':
        return `m:${first.id}`;
      case 'question':
      case 'said':
        return `t:${g.task}`;
      case 'gathr':
        if (!g.turn) return first.topic ?? `m:${first.id}`;
        return ofTurn(g.turn);
      case 'to': {
        const parent = spoken.get(first.id)?.parent;
        return (parent && ofMessage.get(parent)) || ofTurn(g.turn ?? '');
      }
      default: {
        const asked = hosts.get(g);
        if (asked?.length) return `t:${asked[0]}`;
        return g.turn ? ofTurn(g.turn) : `m:${first?.id ?? g.key}`;
      }
    }
  }
}
