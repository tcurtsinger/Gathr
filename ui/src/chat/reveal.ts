import { TOP } from './found';

/**
 * After a press opens something in place under `opener` (a turn's steps, a thread, an ask's sizes), scrolls the chat
 * so `block`, which holds what opened, ends above the message box and its fade, as far as that keeps the opener in
 * view. Again as the opening's motion ends: it grows the block. Outside the chat (an agent's panel) it does nothing.
 */
export function revealOpened(opener: Element, block: Element) {
  const viewport = opener.closest<HTMLElement>('.thread-viewport');
  if (!viewport) return;
  // Where the opener sits in the chat, read now: some openers are replaced by what they open.
  const openerAt = opener.getBoundingClientRect().top + viewport.scrollTop;
  const show = () => {
    const view = viewport.getBoundingClientRect();
    const footer = viewport.querySelector('.thread-footer');
    if (!footer) return;
    const fade = Number.parseFloat(getComputedStyle(footer, '::before').height) || 0;
    const below = block.getBoundingClientRect().bottom - (footer.getBoundingClientRect().top - fade);
    const room = openerAt - viewport.scrollTop - view.top - TOP;
    if (below > 0 && room > 0) viewport.scrollTop += Math.min(below, room);
  };
  requestAnimationFrame(show);
  const ends = ['transitionend', 'animationend'];
  for (const end of ends) block.addEventListener(end, show);
  setTimeout(() => {
    for (const end of ends) block.removeEventListener(end, show);
  }, 1000);
}
