import { useRef, useState } from 'react';
import type { Spot, Timeline } from './messages';

/**
 * Which threads are open while the chat is open (0075). A thread whose main message was handled when the chat opened
 * starts folded, every other open; nothing folds or opens by itself after that, not even for a new reply. The person's
 * own fold or open holds until the chat closes. A search hit or an approval shown from the dock opens its thread,
 * and its earlier replies, once.
 */
export function useFolds() {
  const [folds, setFolds] = useState(() => ({
    open: new Map<string, boolean>(),
    earlier: new Set<string>(),
  }));
  const start = useRef<Set<string>>(undefined);
  const revealed = useRef<Partial<Spot>>(undefined);
  const isOpen = (topic: string) => folds.open.get(topic) ?? !start.current?.has(topic);
  const reveal = (topic: string) =>
    setFolds((f) => ({ open: new Map(f.open).set(topic, true), earlier: new Set(f.earlier).add(topic) }));
  return {
    /** The draw's inputs: they change only with a fold, so a streamed word doesn't make them new. */
    folds,
    start,
    reveal,
    fold: (topic: string) => setFolds((f) => ({ ...f, open: new Map(f.open).set(topic, !isOpen(topic)) })),
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
