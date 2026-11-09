import { type RefObject, useLayoutEffect, useRef, useState } from 'react';

const PAGE = 100;
// How close to the top, in pixels, scrolling brings in the next older page.
const NEAR = 800;

/**
 * The newest messages of a long chat, and older ones a page at a time as the person scrolls up, so
 * opening a chat of hundreds draws only what's near the bottom (slice-2b's 200 ms budget).
 * Adding a page above keeps the messages on screen where they were.
 */
export function useOlder<T>(all: T[], viewport: RefObject<HTMLElement | null>) {
  const [shown, setShown] = useState(PAGE);
  const fromBottom = useRef<number | null>(null);

  useLayoutEffect(() => {
    const el = viewport.current;
    if (!el || fromBottom.current === null || shown === 0) return;
    el.scrollTop = el.scrollHeight - fromBottom.current;
    fromBottom.current = null;
  }, [shown, viewport]);

  const onScroll = () => {
    const el = viewport.current;
    if (!el || shown >= all.length || el.scrollTop > NEAR || fromBottom.current !== null) return;
    fromBottom.current = el.scrollHeight - el.scrollTop;
    setShown((n) => n + PAGE);
  };

  return { messages: all.length > shown ? all.slice(-shown) : all, onScroll };
}
