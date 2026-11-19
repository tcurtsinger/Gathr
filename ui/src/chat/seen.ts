import { type RefObject, useEffect } from 'react';
import { type Reads, SETTLE_MS } from './reads';

/** The window is in front: what shows in it is seen. */
const front = () => document.visibilityState === 'visible' && document.hasFocus();

/**
 * What of the chat is in view, between its top and the message box, and what is seen (0077): half of a message, or half
 * the view's height of one taller than that, while the window is in front. The browser says which items reach the view
 * at all; only those are measured, at most once a frame, as the chat scrolls, grows or comes to the front.
 */
export function useSeen(viewport: RefObject<HTMLElement | null>, reads: Reads | undefined) {
  useEffect(() => {
    const el = viewport.current;
    const list = el?.querySelector('.thread-messages');
    if (!reads || !el || !list) return;
    const near = new Set<Element>();
    let frame = 0;
    let settle = 0;
    const measure = () => {
      frame = 0;
      const view = el.getBoundingClientRect();
      const top = view.top;
      const bottom = Math.min(
        view.bottom,
        el.querySelector('.thread-footer')?.getBoundingClientRect().top ?? view.bottom,
      );
      const shown = new Set<string>();
      const seen: string[] = [];
      const inFront = front();
      for (const item of near) {
        const id = item.getAttribute('data-message-id');
        if (!id) continue;
        const box = item.getBoundingClientRect();
        const showing = Math.min(box.bottom, bottom) - Math.max(box.top, top);
        if (showing <= 0) continue;
        shown.add(id);
        if (inFront && showing >= Math.min(box.height, bottom - top) / 2) seen.push(id);
      }
      // A block came: measured again once the chat has followed its foot to it.
      if (reads.view(shown, seen)) {
        clearTimeout(settle);
        settle = window.setTimeout(look, SETTLE_MS);
      }
    };
    const look = () => {
      frame ||= requestAnimationFrame(measure);
    };
    const reach = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) near.add(e.target);
          else near.delete(e.target);
        }
        look();
      },
      { root: el, threshold: [0, 0.5, 1] },
    );
    const watch = () => {
      for (const item of list.children) reach.observe(item);
      look();
    };
    const added = new MutationObserver((changes) => {
      for (const c of changes) for (const gone of c.removedNodes) near.delete(gone as Element);
      watch();
    });
    added.observe(list, { childList: true });
    const grown = new ResizeObserver(look);
    grown.observe(list);
    watch();
    el.addEventListener('scroll', look, { passive: true });
    window.addEventListener('focus', look);
    document.addEventListener('visibilitychange', look);
    return () => {
      cancelAnimationFrame(frame);
      clearTimeout(settle);
      reach.disconnect();
      added.disconnect();
      grown.disconnect();
      el.removeEventListener('scroll', look);
      window.removeEventListener('focus', look);
      document.removeEventListener('visibilitychange', look);
    };
  }, [reads, viewport]);
}
