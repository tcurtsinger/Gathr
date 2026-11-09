import { type RefObject, useEffect } from 'react';

/**
 * Marks the scroller with which edges have more of the chat past them (`data-fade-above`,
 * `data-fade-below`), for DESIGN.md's canvas fades. Set on the element, not in state, so scrolling
 * never redraws the thread.
 */
export function useEdges(viewport: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = viewport.current;
    if (!el) return;
    const measure = () => {
      el.toggleAttribute('data-fade-above', el.scrollTop > 1);
      el.toggleAttribute('data-fade-below', el.scrollHeight - el.clientHeight - el.scrollTop > 1);
    };
    const sizes = new ResizeObserver(measure);
    sizes.observe(el);
    for (const child of el.children) sizes.observe(child);
    el.addEventListener('scroll', measure, { passive: true });
    return () => {
      sizes.disconnect();
      el.removeEventListener('scroll', measure);
    };
  }, [viewport]);
}
