import type { ThreadMessageLike } from '@assistant-ui/react';
import { memo, type RefObject, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ConversationMap } from '@/components/assistant-ui/elements/conversation-map';
import { mapEntries, type Tick, ticksOf } from './map';
import './map.css';

// Ticks never sit closer than this (0039).
const APART = 6;
// Clear of the header above and the composer below.
const EDGE = 8;
// What the rail needs left of the column: its 14px inset, its 24px, and 6px before the column.
const BESIDE = 44;
const TICK = '[data-slot="conversation-map-tick"]';

interface Props {
  ticks: Tick[];
  /** The chat's scroller: the map reads what shows in it, and never scrolls it. */
  viewport: RefObject<HTMLElement | null>;
  /** Opens a tick's message. */
  onJump: (id: string) => void;
}

/** The chat's ticks: the same list while none of them changed, so a streamed token draws no map again. */
export function useTicks(items: readonly ThreadMessageLike[]): Tick[] {
  const kept = useRef<Tick[]>([]);
  const next = ticksOf(items);
  const same = (a: Tick, b: Tick) =>
    a.id === b.id && a.kind === b.kind && a.who === b.who && a.time === b.time && a.text === b.text;
  if (next.length !== kept.current.length || next.some((t, i) => !same(t, kept.current[i]))) {
    kept.current = next;
  }
  return kept.current;
}

/** The first of `n` places, in order, where `after` holds: it holds from there on. */
function firstWhere(n: number, after: (i: number) => boolean): number {
  let low = 0;
  let high = n;
  while (low < high) {
    const mid = (low + high) >> 1;
    if (after(mid)) high = mid;
    else low = mid + 1;
  }
  return low;
}

/**
 * The conversation map (0039): assistant-ui's rail on the chat's left edge, between the header and the
 * composer. A long tick for each message from the person, a short one for each of the lead's replies to
 * them, earlier days folded while the rail is too short for 6px a tick. It shows from three ticks up, in
 * a pane wide enough to hold it beside the column.
 */
export const ChatMap = memo(function ChatMap({ ticks, viewport, onJump }: Props) {
  const rail = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ bottom: 0, most: 0, fits: false });
  // The ticks on screen, first to last, and the latest one at or above the middle.
  const [seen, setSeen] = useState({ first: 0, last: -1, active: -1 });
  const today = new Date().toDateString();
  // biome-ignore lint/correctness/useExhaustiveDependencies: folded again when the day turns
  const entries = useMemo(() => mapEntries(ticks, box.most, new Date()), [ticks, box.most, today]);
  const shown = box.fits && ticks.length >= 3 && entries.length > 0;

  useLayoutEffect(() => {
    const el = viewport.current;
    const list = el?.querySelector('.thread-messages');
    const footer = el?.querySelector<HTMLElement>('.thread-footer');
    const column = el?.querySelector<HTMLElement>('.thread-column');
    if (!el || !list || !footer || !column) return;
    const places = new Map(ticks.map((t, i) => [t.id, i]));
    // The ticked messages that are drawn, in order; read again once messages come or go.
    let drawn: { block: Element; tick: number }[] | undefined;
    let frame = 0;
    const read = () => {
      frame = 0;
      const bottom = footer.offsetHeight;
      const most = Math.floor((el.clientHeight - bottom - 2 * EDGE) / APART);
      const fits = column.offsetLeft >= BESIDE;
      setBox((b) => (b.bottom === bottom && b.most === most && b.fits === fits ? b : { bottom, most, fits }));

      if (!fits || ticks.length < 3) return;
      drawn ??= [...list.querySelectorAll(':scope > [data-message-id]')].flatMap((block) => {
        const tick = places.get(block.getAttribute('data-message-id') ?? '');
        return tick === undefined ? [] : [{ block, tick }];
      });
      const all = drawn;
      const top = el.getBoundingClientRect().top;
      const foot = footer.getBoundingClientRect().top;
      const rect = (i: number) => all[i].block.getBoundingClientRect();
      // Blocks sit in order down the chat, so each edge is found in a few reads, not one per message.
      const first = firstWhere(all.length, (i) => rect(i).bottom > top);
      const end = firstWhere(all.length, (i) => rect(i).top >= foot);
      const below = firstWhere(all.length, (i) => rect(i).top > (top + foot) / 2);
      // Ticks not drawn are further up: with none drawn above the middle, the latest of those.
      const active = below > 0 ? all[below - 1].tick : Math.max(0, (all[0]?.tick ?? ticks.length) - 1);
      const next =
        end > first
          ? { first: all[first].tick, last: all[end - 1].tick, active }
          : { first: 0, last: -1, active };
      setSeen((s) => (s.first === next.first && s.last === next.last && s.active === active ? s : next));
    };
    const soon = () => {
      frame ||= requestAnimationFrame(read);
    };
    const sizes = new ResizeObserver(soon);
    for (const part of [el, footer, list]) sizes.observe(part);
    const blocks = new MutationObserver(() => {
      drawn = undefined;
      soon();
    });
    blocks.observe(list, { childList: true });
    el.addEventListener('scroll', soon, { passive: true });
    read();
    return () => {
      cancelAnimationFrame(frame);
      sizes.disconnect();
      blocks.disconnect();
      el.removeEventListener('scroll', soon);
    };
  }, [viewport, ticks]);

  // The registry's rail draws every tick alike and gives none a mark of its own, so each gets its kind here.
  useLayoutEffect(() => {
    if (!shown) return;
    for (const [i, tick] of (rail.current?.querySelectorAll(TICK) ?? []).entries()) {
      tick.setAttribute('data-kind', entries[i]?.kind ?? '');
    }
  }, [entries, shown]);

  if (!shown) return null;
  const active = entries.findLast((e) => e.from <= seen.active)?.id;
  const inView = entries
    .filter((e, i) => e.from <= seen.last && (entries[i + 1]?.from ?? ticks.length) > seen.first)
    .map((e) => e.id);
  return (
    <div className="chat-map" ref={rail} style={{ bottom: box.bottom }}>
      <ConversationMap
        entries={entries}
        activeId={active}
        visibleIds={inView}
        onSelect={onJump}
        side="right"
      />
    </div>
  );
});
