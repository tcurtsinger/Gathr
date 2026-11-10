import type { ThreadMessageLike } from '@assistant-ui/react';
import { type RefObject, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { report, timeToPaint } from '../report';
import type { Columns, Spot } from './messages';
import { PAGE } from './older';
import { centre } from './reading';
import type { Target } from './Thread';

const ABOVE = 20;

/**
 * A search hit opened in a column (plans/search.md): draws the pages up to it, centres it and tints it,
 * then holds the view there, so neither a stream nor a new turn pulls it to the bottom, until the person
 * scrolls back to the bottom or sends a message.
 */
export function useFound(
  all: ThreadMessageLike[],
  viewport: RefObject<HTMLElement | null>,
  reach: (index: number) => void,
  target: Target | undefined,
  onTarget: (() => void) | undefined,
) {
  // A column drawn for the hit (the team column opened for it) has it from its first draw.
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
  // Each frame until it's centred: assistant-ui draws the history, and the page it's on, after this column.
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
    const index = all.findIndex((m) => m.id === found.id);
    if (index < 0) return;
    const message = el.querySelector(`[data-message-id="${CSS.escape(found.id)}"]`);
    if (!message) {
      // With messages above it too, so it can sit in the middle.
      reach(Math.max(0, index - ABOVE));
      if (frames > 60) {
        report({
          name: 'gathr.ui.search.error',
          error: 'not_drawn',
          attrs: { from_end: all.length - index },
        });
        return release();
      }
      const frame = requestAnimationFrame(() => nextFrame((n) => n + 1));
      return () => cancelAnimationFrame(frame);
    }
    const block = (found.text && message.querySelector('.found-part')) || message;
    spot.current?.removeAttribute('data-search-hit');
    centre(el, block);
    // A column drawn for it may still wrap its text again once its scroll bar shows.
    const settle = (left: number) =>
      requestAnimationFrame(() => {
        if (spot.current !== block) return;
        centre(el, block);
        if (left > 1) settle(left - 1);
      });
    settle(2);
    block.setAttribute('data-search-hit', '');
    // Keys scroll the chat from here, and a screen reader reads the message.
    if (block instanceof HTMLElement) {
      block.tabIndex = -1;
      block.focus({ preventScroll: true });
    }
    spot.current = block;
    centred.current = found;
    sent.current = lastSent;
    nextFrame(0);
    const fromEnd = all.length - Math.max(0, index - ABOVE);
    timeToPaint('gathr.ui.search.open', found.at, {
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
    if (el && centred.current && el.scrollHeight - el.clientHeight - el.scrollTop < 2) release();
  };
  return { held: !!found, text: found?.text, onScroll };
}

/**
 * Where a hit's message shows, once the chat is loaded: the team column opens for one there. A message
 * no longer in the chat leaves it at the bottom, with `gathr.ui.search.error`.
 */
export function useTarget(
  found: { message: string; at: number } | undefined,
  loaded: boolean,
  columns: Columns,
  split: boolean,
  onSplit: (open: boolean) => void,
  onFound: (() => void) | undefined,
): (Spot & Target) | undefined {
  // biome-ignore lint/correctness/useExhaustiveDependencies: placed once, as the chat is when it loads
  const target = useMemo(() => {
    const spot = found && loaded ? columns.find(found.message) : undefined;
    return spot && found && { ...spot, at: found.at };
  }, [found, loaded]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: once per hit
  useEffect(() => {
    if (!found || !loaded) return;
    if (!target) {
      report({ name: 'gathr.ui.search.error', error: 'message_gone', attrs: { message: found.message } });
      onFound?.();
    } else if (target.column === 'team' && !split) onSplit(true);
  }, [target, loaded]);
  return target;
}
