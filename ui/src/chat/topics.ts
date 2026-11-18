import type { Group } from './groups';
import type { ChatState } from './state';
import type { Spoken } from './threads';

/**
 * The main message each block sits under (0075). The service names it for every turn, task and line said in no turn
 * (`topic`); the window keeps its two rules inside a turn: a turn splits where the person's message entered it, and
 * the blocks after take that message's topic; a message between agents that answers one Gathr carried into its turn
 * sits where what it answers sits. A turn the service hasn't named yet is its own main message until it does.
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
  // In saved order: a split applies to the blocks saved after it, and a reply after what it answers.
  for (const g of [...groups].sort((a, b) => a.at - b.at)) {
    const topic = topicOf(g);
    topics.set(g, topic);
    for (const m of g.msgs) ofMessage.set(m.id, topic);
  }
  return topics;

  function topicOf(g: Group): string {
    const first = g.msgs[0];
    switch (g.kind) {
      case 'person':
        // A person's message into a running turn: what the turn says after it is about it.
        if (g.turn && !g.bound) entered.set(g.turn, `m:${first.id}`);
        return `m:${first.id}`;
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
