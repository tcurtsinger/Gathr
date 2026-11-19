import { useRef, useState } from 'react';
import type { Spot, Timeline } from './messages';
import type { Reads } from './reads';

/**
 * Which threads are open (0075, 0077). When the chat opens, a thread whose main message is handled and whose replies
 * the person has all seen starts folded, every other open; nothing folds or opens by itself after that, not even for a
 * new reply. The person's own fold or open is kept by the service (`reads`) and wins, in every window and after a
 * reload. A search hit, an approval Summary goes to, or the New pill opens its thread, and its earlier
 * replies, for as long as the chat is open.
 */
export function useFolds(reads?: Reads) {
  const [folds, setFolds] = useState(() => ({
    open: new Map<string, boolean>(),
    earlier: new Set<string>(),
  }));
  const start = useRef<Set<string>>(undefined);
  const revealed = useRef<Partial<Spot>>(undefined);
  const kept = (topic: string) => {
    const folded = reads?.folds.get(topic);
    return folded === undefined ? undefined : !folded;
  };
  const isOpen = (topic: string) => folds.open.get(topic) ?? kept(topic) ?? !start.current?.has(topic);
  const reveal = (topic: string) =>
    setFolds((f) => ({ open: new Map(f.open).set(topic, true), earlier: new Set(f.earlier).add(topic) }));
  return {
    /** The draw's inputs: they change only with a fold, so a streamed word doesn't make them new. */
    folds,
    start,
    reveal,
    /** The person's own fold or open: kept. */
    fold: (topic: string) => {
      const open = isOpen(topic);
      reads?.fold(topic, open);
      setFolds((f) => ({ ...f, open: new Map(f.open).set(topic, !open) }));
    },
    /** The person folded or opened it in another window: that wins over what this one held. */
    drop: (topic: string) =>
      setFolds((f) => {
        if (!f.open.has(topic)) return f;
        const open = new Map(f.open);
        open.delete(topic);
        return { ...f, open };
      }),
    /** Opens a thread to reply in it (0076); one already open stays as it is. */
    show: (topic: string) => {
      if (!isOpen(topic)) setFolds((f) => ({ ...f, open: new Map(f.open).set(topic, true) }));
    },
    earlier: (topic: string) => setFolds((f) => ({ ...f, earlier: new Set(f.earlier).add(topic) })),
    /** Called with each draw: the first once the chat has loaded fixes what starts folded. */
    settle: (timeline: Timeline, loaded: boolean) => {
      if (loaded && !start.current) start.current = timeline.handled;
    },
    /** A hit in a folded thread, or behind its earlier replies: opened during the draw, so it is there to centre. */
    open: (hit: Partial<Spot> | undefined) => {
      if (!hit?.topic || revealed.current === hit) return;
      revealed.current = hit;
      if (!isOpen(hit.topic) || !folds.earlier.has(hit.topic)) reveal(hit.topic);
    },
  };
}
