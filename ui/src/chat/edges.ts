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
    // Content can arrive after this ran (main's WebKit run, c301c38): each new child is watched too,
    // or the chat could grow with nothing measuring it.
    const added = new MutationObserver((changes) => {
      for (const change of changes)
        for (const node of change.addedNodes) if (node instanceof Element) sizes.observe(node);
      measure();
    });
    added.observe(el, { childList: true });
    el.addEventListener('scroll', measure, { passive: true });
    return () => {
      sizes.disconnect();
      added.disconnect();
      el.removeEventListener('scroll', measure);
    };
  }, [viewport]);
}
