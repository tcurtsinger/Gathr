import { type RefObject, useEffect } from 'react';

/**
 * Focus moving up the chat, to something above what shows (Tab, Shift+Tab), takes the view to it now and tells the
 * runtime at once that the view moved. The runtime learns it has left the chat's foot only from the scroll's own event,
 * a frame later, and WebKit scrolls to what has focus later still: anything drawn at the foot before then made it follow
 * its foot and take the view back down, away from what had focus. In the middle of what shows, as the browsers' own
 * focus does.
 */
export function useFocusUp(viewport: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = viewport.current;
    if (!el) return;
    // Where the last scroll event left the view: Chromium has scrolled to what has focus by the time it hears of it.
    let told = el.scrollTop;
    const onScroll = () => {
      told = el.scrollTop;
    };
    const onFocus = (e: FocusEvent) => {
      const target = e.target as Element;
      if (target === el || target.closest('.thread-footer')) return;
      const view = el.getBoundingClientRect();
      const box = target.getBoundingClientRect();
      // One taller than the view is left to the browser.
      if (box.top < view.top && box.height <= view.height) target.scrollIntoView({ block: 'center' });
      if (el.scrollTop < told) el.dispatchEvent(new Event('scroll'));
    };
    el.addEventListener('scroll', onScroll, { passive: true });
    el.addEventListener('focusin', onFocus);
    return () => {
      el.removeEventListener('scroll', onScroll);
      el.removeEventListener('focusin', onFocus);
    };
  }, [viewport]);
}
