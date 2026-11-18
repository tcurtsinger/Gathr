import type { Message, Turn } from '../api';
import type { Work } from './fold';
import type { Group } from './groups';
import type { Like } from './likes';
import type { ChatState } from './state';
import type { Team } from './team';

/** One message between agents as its item draws it. */
export interface Said {
  id: string;
  from?: string;
  /** The writer's id, for its face, removed or not. */
  agent?: string;
  /** The writer's id while it is in the chat: its face opens its panel. */
  by?: string;
  /** Who it is to, by name: the blue @Name it opens with. None when it quotes what it answers. */
  to?: string;
  /** What it answers, when something came between the two. */
  quote?: string;
  time: string;
  agreed: string[];
  /** The person's like (0057). */
  like?: Like;
  /** What the writer's turn did before it, when that turn drew nothing else. */
  work?: Work;
}

/** A message's words as drawn: an @Name it opens with goes, as the name is drawn before them. */
export interface Spoken {
  /** The message Gathr carried into its turn that it answers. */
  parent?: string;
  quote?: string;
  mention?: string;
  text: string;
}

const literal = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/**
 * Messages between agents (0072). A message answers the one Gathr carried into its turn from the agent it writes to
 * (the copy in its turn), and sits in that one's thread, quoting it when something came between the two. A block of
 * a turn that wrote no words and only talked (`quiet`) isn't drawn: its steps go on the turn's first message.
 */
export function talks(groups: Group[], state: ChatState, crew: Team, hasNotice: (t: Turn) => boolean) {
  const byId = new Map(state.messages.map((m) => [m.id, m]));
  const sender = (m: Message | undefined) => (m ? state.turns[m.turn ?? '']?.agent : undefined);
  const carried = new Map<string, string[]>();
  const parents = new Map<string, string>();
  for (const m of state.messages) {
    if (m.role === 'gathr' && m.copy_of && m.turn)
      carried.set(m.turn, [...(carried.get(m.turn) ?? []), m.copy_of]);
    if (m.role !== 'assistant' || !m.to) continue;
    const parent = (carried.get(m.turn ?? '') ?? []).findLast((id) => sender(byId.get(id)) === m.to);
    if (parent) parents.set(m.id, parent);
  }
  const firstOf = new Map<string, Group>();
  for (const g of groups) if (g.kind === 'to' && g.turn && !firstOf.has(g.turn)) firstOf.set(g.turn, g);
  const waiting = new Set(state.approvals.map((a) => a.turn));
  const quiet = new Map<Group, Group>();
  for (const g of groups) {
    const first = g.kind === 'reply' && g.turn ? firstOf.get(g.turn) : undefined;
    if (!first || g.msgs.some((m) => m.kind === 'text')) continue;
    const t = state.turns[g.turn ?? ''];
    if (g.last && (state.partial[t?.id ?? ''] || (t && hasNotice(t)) || waiting.has(t?.id ?? ''))) continue;
    quiet.set(g, first);
  }
  const spoken = new Map<string, Spoken>();
  let before: string | undefined;
  for (const g of groups) {
    if (g.kind !== 'to') continue;
    const m = g.msgs[0];
    const parent = parents.get(m.id);
    const quote = parent && parent !== before ? (byId.get(parent)?.body as string) : undefined;
    before = m.id;
    const to = crew.name(m.to);
    const body = m.body as string;
    if (quote || !to) {
      spoken.set(m.id, { parent, quote, text: body });
      continue;
    }
    const text = body.replace(new RegExp(`^@${literal(to)}(?![\\w-])\\s*`, 'i'), '');
    spoken.set(m.id, { parent, mention: to, text });
  }
  return { quiet, spoken };
}
