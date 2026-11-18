import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Message, Turn } from '../api';
import { fold, type Given, type Work } from './fold';
import { type Like, saidLike } from './likes';
import type { Cache, Custom, Group } from './messages';
import type { ChatState } from './state';
import type { Team } from './team';

/** One message between agents as its thread draws it. */
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
  /** The person's like (0057); a whole block carries its own. */
  like?: Like;
  /** What the writer's turn did before it, when that turn drew nothing else. */
  work?: Work;
  /** A whole block of a turn, as it reads elsewhere: a review or a fix under its PR (reviews.ts). */
  block?: Custom;
  /** That block's turn is running. */
  running?: boolean;
}

/** Talk between agents: the block it belongs under (none: it stands where it began) and its messages, in order. */
export interface Talk {
  root?: Group;
  groups: Group[];
}

/** A message's words as drawn: an @Name it opens with goes, as the name is drawn before them. */
export interface Spoken {
  parent?: string;
  quote?: string;
  mention?: string;
  text: string;
}

const literal = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/**
 * Which talk each message between agents belongs to (0072). A message answers the one Gathr carried into its turn
 * from the agent it writes to (the copy in its turn), and joins that one's thread; a message that answers none
 * belongs under the block its own turn was writing when it was sent, with the others its turn sent. A block of a
 * turn that wrote no words and only talked (`quiet`) isn't drawn: its steps go on the turn's first message.
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
  const of = new Map<Group, Talk>();
  const byMessage = new Map<string, Talk>();
  const byRoot = new Map<Group | string, Talk>();
  const firstOf = new Map<string, Group>();
  for (const g of groups) {
    if (g.kind !== 'to') continue;
    const m = g.msgs[0];
    const parent = parents.get(m.id);
    const key = g.root ?? `turn:${m.turn}`;
    let talk = (parent && byMessage.get(parent)) || byRoot.get(key);
    if (!talk) {
      talk = { root: g.root, groups: [] };
      byRoot.set(key, talk);
    }
    talk.groups.push(g);
    of.set(g, talk);
    byMessage.set(m.id, talk);
    if (m.turn && !firstOf.has(m.turn)) firstOf.set(m.turn, g);
  }
  const waiting = new Set(state.approvals.map((a) => a.turn));
  const quiet = new Map<Group, Group>();
  for (const g of groups) {
    const first = g.kind === 'reply' && g.turn ? firstOf.get(g.turn) : undefined;
    if (!first || g.msgs.some((m) => m.kind === 'text')) continue;
    const t = state.turns[g.turn ?? ''];
    if (g.last && (state.partial[t?.id ?? ''] || (t && hasNotice(t)) || waiting.has(t?.id ?? ''))) continue;
    quiet.set(g, first);
  }
  for (const talk of new Set(of.values())) if (talk.root && quiet.has(talk.root)) talk.root = undefined;
  const spoken = new Map<string, Spoken>();
  for (const talk of new Set(of.values())) {
    for (const [i, g] of talk.groups.entries()) {
      const m = g.msgs[0];
      const parent = parents.get(m.id);
      const before = talk.groups[i - 1]?.msgs[0].id;
      const quote = parent && parent !== before ? (byId.get(parent)?.body as string) : undefined;
      const to = crew.name(m.to);
      const body = m.body as string;
      if (quote || !to) {
        spoken.set(m.id, { parent, quote, text: body });
        continue;
      }
      const text = body.replace(new RegExp(`^@${literal(to)}(?![\\w-])\\s*`, 'i'), '');
      spoken.set(m.id, { parent, mention: to, text });
    }
  }
  return { of, quiet, spoken };
}

/** A talk as one item: each message's words, and who wrote it to whom. Made again only when it changed. */
export function threadItem(
  cache: Cache,
  talk: Talk,
  state: ChatState,
  crew: Team,
  {
    quiet,
    spoken,
    gave,
    agreedBy,
  }: {
    quiet: Map<Group, Group>;
    spoken: Map<string, Spoken>;
    gave: Map<string, Given>;
    agreedBy: (message: string) => string[];
  },
): ThreadMessageLike {
  const key = talk.groups[0].key;
  const worked = new Map([...quiet].map(([work, to]) => [to, work]));
  const said = talk.groups.map((g): Said => {
    const m = g.msgs[0];
    const agent = state.turns[m.turn ?? '']?.agent ?? undefined;
    const { quote, mention } = spoken.get(m.id) ?? {};
    const work = worked.get(g);
    const turn = work?.turn ? state.turns[work.turn] : undefined;
    const running = !!work?.last && turn?.status === 'running';
    const did = work && fold(work.msgs, '', running, gave, work.bound ? turn?.created : undefined).work;
    return {
      id: m.id,
      from: crew.name(agent),
      agent,
      by: state.members.some((x) => x.agent === agent) ? agent : undefined,
      to: mention,
      quote,
      time: m.time,
      agreed: agreedBy(m.id),
      like: saidLike(state, m.id),
      ...(did && { work: did }),
    };
  });
  const texts = talk.groups.map((g) => spoken.get(g.msgs[0].id)?.text ?? (g.msgs[0].body as string));
  const sig = JSON.stringify([said, texts, !!talk.root]);
  const hit = cache.get(key);
  if (hit?.sig === sig) return hit.value;
  const custom: Custom = { thread: said, under: !!talk.root };
  const value: ThreadMessageLike = {
    id: key,
    role: 'assistant',
    content: texts.map((text) => ({ type: 'text', text })),
    metadata: { custom },
  };
  cache.set(key, { sig, value });
  return value;
}
