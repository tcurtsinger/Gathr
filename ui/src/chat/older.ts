import { type RefObject, useState } from 'react';

export const PAGE = 100;
// How close to the top, in pixels, scrolling brings in the next older page.
const NEAR = 800;

/**
 * The newest messages of a long chat, and older ones a page at a time as the person scrolls up, so
 * opening a chat of hundreds draws only what's near the bottom (slice-2b's 200 ms budget).
 * assistant-ui's viewport keeps the message the person was reading where it was when a page is added above.
 */
export function useOlder<T>(all: T[], viewport: RefObject<HTMLElement | null>) {
  const [shown, setShown] = useState(PAGE);

  const onScroll = () => {
    const el = viewport.current;
    if (!el || shown >= all.length || el.scrollTop > NEAR) return;
    // Scroll events before the next draw add one page, not one each.
    setShown((n) => Math.max(n, shown + PAGE));
  };

  /** Draws everything from the item at `index` on, for a link to a message further up. */
  const reach = (index: number) => setShown((n) => Math.max(n, all.length - index));

  return { messages: all.length > shown ? all.slice(-shown) : all, onScroll, reach };
}
