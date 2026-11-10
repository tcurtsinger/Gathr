/** What the reader of a scroller is looking at: the first block in view, and where it starts. */
export interface Reading {
  viewport: HTMLElement;
  at: Element;
  offset: number;
  /** The scroller's position once the block was kept there. */
  kept?: number;
}

// Below the sticky column label, which covers the top of a split column (DESIGN.md).
const BELOW_LABEL = 56;
// Blocks keep their first line where it was; a message with none (a tool row, a notice) keeps its top.
const BLOCKS = '[data-message-id], [data-message-id] :is(p, li, pre, h1, h2, h3, h4, blockquote, table)';

/**
 * The block being read at the top of a column, to keep there when its width changes. The one kept last
 * time if the column hasn't scrolled since, so widening then narrowing comes back to the same place.
 * None at the bottom: there assistant-ui follows the latest reply, which a held block would stop.
 */
export function readingAt(viewport: HTMLElement | null, last: Reading | null): Reading | null {
  if (!viewport || viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop < 2) return null;
  if (last?.viewport === viewport && last.kept !== undefined && Math.abs(last.kept - viewport.scrollTop) < 1)
    return last;
  const top = viewport.getBoundingClientRect().top;
  for (const at of viewport.querySelectorAll(BLOCKS)) {
    const offset = at.getBoundingClientRect().top - top;
    if (offset >= BELOW_LABEL) return { viewport, at, offset };
  }
  return null;
}

/**
 * Scrolls a block to the middle of what shows above the composer; one taller than that starts at the top,
 * below the column label.
 */
export function centre(viewport: HTMLElement, block: Element) {
  const view = viewport.getBoundingClientRect();
  const footer = viewport.querySelector('.thread-footer')?.getBoundingClientRect().height ?? 0;
  const room = view.height - footer;
  const box = block.getBoundingClientRect();
  const offset = box.height > room - BELOW_LABEL ? BELOW_LABEL : (room - box.height) / 2;
  viewport.scrollTop += box.top - view.top - offset;
}

/** Scrolls so the block read before starts where it did. */
export function keepReading(reading: Reading) {
  const { viewport, at, offset } = reading;
  const moved = at.getBoundingClientRect().top - viewport.getBoundingClientRect().top - offset;
  if (Math.abs(moved) >= 1) viewport.scrollTop += moved;
  reading.kept = viewport.scrollTop;
}
