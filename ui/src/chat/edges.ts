import { type RefObject, useEffect } from 'react';

/**
 * Marks the scroller when more of the chat lies past its top edge (`data-fade-above`), for DESIGN.md's canvas fade.
 * Set on the element, not in state, so scrolling never redraws the thread. Only a scroll moves the top edge.
 */
export function useEdges(viewport: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = viewport.current;
    if (!el) return;
    const measure = () => {
      el.toggleAttribute('data-fade-above', el.scrollTop > 1);
    };
    measure();
    el.addEventListener('scroll', measure, { passive: true });
    return () => el.removeEventListener('scroll', measure);
  }, [viewport]);
}
