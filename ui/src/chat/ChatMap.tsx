import type { ThreadMessageLike } from '@assistant-ui/react';
import { memo, type RefObject, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ConversationMap } from '@/components/assistant-ui/elements/conversation-map';
import { mapEntries, type Tick, ticksOf } from './map';
import './map.css';

// Ticks never sit closer than this (0039).
const APART = 6;
// The rail is at most half the pane's height, and never longer than this: 44 ticks at their widest.
const LONGEST = 616;
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

/**
 * The chat's ticks: the same list while none of them changed. A turn has no tick until it ends, so a
 * reply's streamed tokens leave the list, and the map drawn from it, as they were.
 */
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
 * The conversation map (0039): assistant-ui's rail on the chat's left edge, centred in the pane's height
 * and at most half of it. A long tick for each message from the person, a short one for each of the
 * lead's replies to them, earlier days folded while the rail is too short for 6px a tick. It shows from
 * three ticks up, in a pane wide enough to hold it beside the column.
 */
export const ChatMap = memo(function ChatMap({ ticks, viewport, onJump }: Props) {
  const rail = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ most: 0, fits: false });
  // The ticks on screen, first to last, and the current one.
  const [seen, setSeen] = useState({ first: 0, last: -1, active: -1 });
  // A day's name and the fold go by today: a draw on a later day works them out again.
  const today = new Date().toDateString();
  // biome-ignore lint/correctness/useExhaustiveDependencies: today stands for the date the fold reads
  const entries = useMemo(() => mapEntries(ticks, box.most, new Date()), [ticks, box.most, today]);
  const shown = box.fits && ticks.length >= 3 && entries.length > 0;
  // Which messages have ticks: what the tracking below goes by, whatever else of a tick changes.
  const ids = useMemo(() => ticks.map((t) => t.id).join('\n'), [ticks]);

  useLayoutEffect(() => {
    const el = viewport.current;
    const list = el?.querySelector('.thread-messages');
    const footer = el?.querySelector<HTMLElement>('.thread-footer');
    const column = el?.querySelector<HTMLElement>('.thread-column');
    if (!el || !list || !footer || !column) return;
    const places = new Map(ids.split('\n').map((id, i) => [id, i]));
    // The ticked messages that are drawn, in order; read again once messages come or go.
    let drawn: { block: Element; tick: number }[] | undefined;
    let frame = 0;
    const read = () => {
      frame = 0;
      const most = Math.floor(Math.min(el.clientHeight / 2, LONGEST) / APART);
      const fits = column.offsetLeft >= BESIDE;
      setBox((b) => (b.most === most && b.fits === fits ? b : { most, fits }));

      if (!fits || places.size < 3) return;
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
      const atEnd = el.scrollHeight - el.clientHeight - el.scrollTop < 2;
      const active =
        // At the chat's end nothing more can reach the middle: the latest message on screen is current.
        atEnd && end > first
          ? all[end - 1].tick
          : below > 0
            ? all[below - 1].tick
            : // Ticks not drawn are further up: with none drawn above the middle, the latest of those.
              Math.max(0, (all[0]?.tick ?? places.size) - 1);
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
  }, [viewport, ids]);

  // The registry's rail draws every tick alike and names it by its title alone, so each gets its kind
  // here, for map.css, and its preview as its description, for a screen reader.
  useLayoutEffect(() => {
    if (!shown) return;
    for (const [i, tick] of (rail.current?.querySelectorAll(TICK) ?? []).entries()) {
      tick.setAttribute('data-kind', entries[i]?.kind ?? '');
      tick.setAttribute('aria-description', entries[i]?.preview ?? '');
    }
  }, [entries, shown]);

  if (!shown) return null;
  const active = entries.findLast((e) => e.from <= seen.active)?.id;
  const inView = entries
    .filter((e, i) => e.from <= seen.last && (entries[i + 1]?.from ?? ticks.length) > seen.first)
    .map((e) => e.id);
  return (
    <div className="chat-map" ref={rail}>
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
