import type { ThreadMessageLike } from '@assistant-ui/react';
import { type RefObject, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { report, timeToPaint } from '../report';
import type { Spot, Timeline } from './messages';
import { PAGE } from './older';
import type { Target } from './Thread';

const ABOVE = 20;
// A block taller than the view starts this far below its top edge, clear of the fade.
const TOP = 32;

/**
 * A search hit opened in the chat (plans/search.md), or an approval shown from the dock: draws the pages
 * up to it, centres it and tints it, then holds the view there, so neither a stream nor a new turn pulls
 * it to the bottom, until the person scrolls back to the bottom or sends a message. A tick of the
 * conversation map opens its message the same way, at the top of the view and untinted (0039).
 */
export function useFound(
  all: ThreadMessageLike[],
  viewport: RefObject<HTMLElement | null>,
  reach: (index: number) => void,
  target: Target | undefined,
  onTarget: (() => void) | undefined,
) {
  const [found, setFound] = useState<Target | null>(target ?? null);
  const [seen, setSeen] = useState(target);
  if (target !== seen) {
    setSeen(target);
    if (target) setFound(target);
  }
  const centred = useRef<Target | null>(null);
  const spot = useRef<Element | null>(null);
  const lastSent = all.findLast((m) => m.role === 'user')?.id;
  const sent = useRef(lastSent);
  // Each frame until it's centred: assistant-ui draws the history, and the page it's on, after the thread.
  const [frames, nextFrame] = useState(0);

  const release = () => {
    nextFrame(0);
    spot.current?.removeAttribute('data-search-hit');
    spot.current = null;
    centred.current = null;
    setFound(null);
  };

  useLayoutEffect(() => {
    const el = viewport.current;
    if (!found || centred.current === found || !el) return;
    if (!found.id) {
      // A chat's title: its latest message, whatever an earlier hit held.
      release();
      el.scrollTop = el.scrollHeight;
      onTarget?.();
      return;
    }
    const index = all.findIndex((m) => m.id === found.id);
    if (index < 0) return;
    const message = el.querySelector(`[data-message-id="${CSS.escape(found.id)}"]`);
    if (!message) {
      // With messages above it too, so it can sit in the middle.
      reach(Math.max(0, index - ABOVE));
      if (frames > 60) {
        report({
          name: found.top ? 'gathr.ui.map.error' : 'gathr.ui.search.error',
          error: 'not_drawn',
          attrs: { from_end: all.length - index },
        });
        return release();
      }
      const frame = requestAnimationFrame(() => nextFrame((n) => n + 1));
      return () => cancelAnimationFrame(frame);
    }
    const card = found.card && message.querySelector(`[data-approval="${CSS.escape(found.card)}"]`);
    const block = card || (found.text && message.querySelectorAll('.found-part')[found.nth ?? 0]) || message;
    spot.current?.removeAttribute('data-search-hit');
    place(el, block, found.top);
    // Its text may still wrap again once the scroll bar shows.
    const settle = (left: number) =>
      requestAnimationFrame(() => {
        if (spot.current !== block) return;
        place(el, block, found.top);
        if (left > 1) settle(left - 1);
      });
    settle(2);
    // A tick's message is the one at the top, and focus stays on the map to move along it.
    if (!found.top) block.setAttribute('data-search-hit', '');
    // Keys scroll the chat from here, and a screen reader reads the message.
    if (!found.top && block instanceof HTMLElement) {
      block.tabIndex = -1;
      block.focus({ preventScroll: true });
    }
    spot.current = block;
    centred.current = found;
    sent.current = lastSent;
    nextFrame(0);
    const fromEnd = all.length - Math.max(0, index - ABOVE);
    if (!found.card)
      timeToPaint(found.top ? 'gathr.ui.map.open' : 'gathr.ui.search.open', found.at, {
        from_end: fromEnd,
        older_pages: Math.ceil(Math.max(0, fromEnd - PAGE) / PAGE),
      });
    onTarget?.();
  });

  // The person's own message goes where they can see it.
  // biome-ignore lint/correctness/useExhaustiveDependencies: only a new message of theirs ends the hold
  useEffect(() => {
    if (sent.current === lastSent) return;
    sent.current = lastSent;
    if (!centred.current) return;
    release();
    const el = viewport.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lastSent]);

  const onScroll = () => {
    const el = viewport.current;
    // Only the target in place: one still on its way there keeps its hold while its pages are drawn.
    if (el && centred.current === found && found && el.scrollHeight - el.clientHeight - el.scrollTop < 2)
      release();
  };
  return { held: !!found, text: found?.text, onScroll };
}

/**
 * Scrolls a block to the middle of what shows above the composer; one taller than that, or one opened
 * from the map, starts at the top.
 */
function place(viewport: HTMLElement, block: Element, top?: boolean) {
  const view = viewport.getBoundingClientRect();
  const footer = viewport.querySelector('.thread-footer')?.getBoundingClientRect().height ?? 0;
  const room = view.height - footer;
  const box = block.getBoundingClientRect();
  const offset = top || box.height > room - TOP ? TOP : (room - box.height) / 2;
  viewport.scrollTop += box.top - view.top - offset;
}

/**
 * The target picked last, of one picked here (a tick, the dock's Show) and one handed down (a search hit).
 * A newer one from above drops the one picked here: when that hit is done and cleared, the view stays on
 * it instead of going back to what was picked before it.
 */
export function useLatest(above: Target | undefined) {
  const [own, pick] = useState<Target>();
  const [seen, setSeen] = useState(above);
  if (above !== seen) {
    setSeen(above);
    if (above) pick(undefined);
  }
  return { target: own ?? above, own: !!own, pick };
}

/**
 * Where a hit's message shows, once the chat is loaded. A message no longer in the chat leaves it at the
 * bottom, with `gathr.ui.search.error`. A chat's title, with no message, opens at its latest.
 */
export function useTarget(
  found: { message?: string; at: number } | undefined,
  loaded: boolean,
  timeline: Timeline,
  onFound: (() => void) | undefined,
): (Partial<Spot> & Target) | undefined {
  // biome-ignore lint/correctness/useExhaustiveDependencies: placed once, as the chat is when it loads
  const target = useMemo<(Partial<Spot> & Target) | undefined>(() => {
    if (!found?.message) return found && { at: found.at };
    const spot = loaded ? timeline.find(found.message) : undefined;
    return spot && { ...spot, at: found.at };
  }, [found, loaded]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: once per hit
  useEffect(() => {
    if (!found?.message || !loaded || target) return;
    report({ name: 'gathr.ui.search.error', error: 'message_gone', attrs: { message: found.message } });
    onFound?.();
  }, [target, loaded]);
  return target;
}
