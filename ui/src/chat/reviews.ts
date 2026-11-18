import type { ThreadMessageLike } from '@assistant-ui/react';
import type { Cache, Custom, Group } from './messages';
import type { ChatState } from './state';
import type { Said } from './threads';

/**
 * Which blocks read under the pull request they are about (0072): a review's turns, and later work on the same PR
 * (its fix), each under the last block of the PR's own task drawn before the first of them. The PR's own task is
 * its earliest work task not cancelled, as the service names the PR's builder. Without one drawn, they stand where
 * they began; so does a block that isn't drawn. One walk over the chat in order.
 */
export function underPrs(groups: Group[], state: ChatState, drawn: (g: Group) => boolean): Map<Group, Group> {
  const tasks = new Map(state.tasks.map((t) => [t.number, t]));
  const own = new Map<number, number>();
  for (const t of state.tasks) {
    if (t.kind !== 'work' || t.pr == null || !t.owner || t.state === 'cancelled') continue;
    if (t.number < (own.get(t.pr) ?? Number.POSITIVE_INFINITY)) own.set(t.pr, t.number);
  }
  const latest = new Map<number, Group>();
  const roots = new Map<number, Group>();
  const under = new Map<Group, Group>();
  for (const g of groups) {
    const t = g.kind === 'reply' ? tasks.get(state.turns[g.turn ?? '']?.task ?? -1) : undefined;
    if (t?.pr == null || (t.kind !== 'work' && t.kind !== 'review') || !drawn(g)) continue;
    const mine = own.get(t.pr);
    if (t.number === mine) {
      if (!roots.has(t.pr)) latest.set(t.number, g);
      continue;
    }
    const root = roots.get(t.pr) ?? latest.get(mine ?? -1);
    if (!root) continue;
    roots.set(t.pr, root);
    under.set(g, root);
  }
  return under;
}

/** Each object's number for this page's life: a block made again is a new object, so the sig moves with it. */
const numbers = new WeakMap<object, number>();
let counted = 0;
const numberOf = (o: object) => {
  const n = numbers.get(o) ?? counted++;
  numbers.set(o, n);
  return n;
};

/**
 * The work on a PR as one thread under its message: each block whole, as it stands elsewhere, then the talk between
 * agents it began. Settled, every task in it done or cancelled, it starts folded. Made again only when one of its
 * blocks was.
 */
export function prThread(
  cache: Cache,
  root: Group,
  items: ThreadMessageLike[],
  settled: boolean,
): ThreadMessageLike {
  const key = `pr-${root.key}`;
  const sig = `${items.map(numberOf)}|${settled}`;
  const hit = cache.get(key);
  if (hit?.sig === sig) return hit.value;
  const said: Said[] = [];
  const content: { type: 'text'; text: string }[] = [];
  for (const item of items) {
    const own = item.metadata?.custom as Custom;
    const parts = (item.content ?? []) as readonly { text?: string }[];
    if (own.thread) {
      said.push(...own.thread);
      content.push(...parts.map((p) => ({ type: 'text' as const, text: p.text ?? '' })));
      continue;
    }
    said.push({
      id: item.id as string,
      from: own.agent,
      agent: own.by,
      by: own.by,
      time: own.time ?? '',
      agreed: [],
      block: own,
      running: item.status?.type === 'running',
    });
    // One part for each message, as the thread draws each by its index.
    content.push({ type: 'text', text: parts[0]?.text ?? '' });
  }
  const custom: Custom = { thread: said, under: true, settled };
  const value: ThreadMessageLike = { id: key, role: 'assistant', content, metadata: { custom } };
  cache.set(key, { sig, value });
  return value;
}
