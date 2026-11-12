import type { Group } from './messages';
import type { ChatState } from './state';

/**
 * The chat's blocks in the order they happened: where each was saved, except that what opens a turn (the
 * message that started it, its first block) never sits below a turn that began after it. Saved order
 * alone isn't that: replies streaming at once save as each finishes, and live updates can arrive out of
 * order, so the service's `position` decides between turns. A turn not started yet has none, and stays
 * where its message was sent.
 */
export function place(groups: Group[], state: ChatState): Group[] {
  const position = (g: Group) => (g.bound && g.turn ? state.turns[g.turn]?.position : undefined) ?? undefined;
  const placed: Group[] = [];
  let latest = Number.NEGATIVE_INFINITY;
  for (const g of [...groups].sort((a, b) => (a.at === b.at ? 0 : a.at < b.at ? -1 : 1))) {
    const at = position(g);
    if (at === undefined || at >= latest) {
      placed.push(g);
      if (at !== undefined) latest = at;
      continue;
    }
    // It began before a turn already placed: it goes above the first of those.
    const later = placed.findIndex((p) => p.turn !== g.turn && (position(p) ?? at) > at);
    placed.splice(later < 0 ? placed.length : later, 0, g);
  }
  return placed;
}
