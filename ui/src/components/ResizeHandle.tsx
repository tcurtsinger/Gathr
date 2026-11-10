import { type KeyboardEvent, type PointerEvent, useLayoutEffect, useRef, useState } from 'react';
import { keep, kept } from '../kept';
import './resize.css';

interface Props {
  label: string;
  /** The pane's edge it sits on: on the left edge, moving left widens the pane. */
  side: 'left' | 'right';
  /** The root's CSS variable that sizes the pane; the window keeps it under the same name. */
  name: string;
  min: number;
  /** Read at each move, as it depends on the window's width. */
  max: () => number;
}

const cookie = (name: string) => `gathr.width${name}`;
const steps: Record<string, number> = { ArrowLeft: -10, ArrowRight: 10 };

/**
 * DESIGN.md Layout: drags with pointer capture, 10px arrows, 40px with Shift, Home and End. It writes
 * the variable straight onto the root, so a drag redraws the layout and never re-renders the chat.
 */
export function ResizeHandle({ label, side, name, min, max }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; width: number } | null>(null);
  const [now, setNow] = useState<number>();
  const root = document.documentElement.style;
  const sign = side === 'left' ? -1 : 1;

  // The kept width before the pane draws; then the pane's real width, after its CSS limits.
  useLayoutEffect(() => {
    const width = Number(kept(cookie(name)));
    if (width > 0) document.documentElement.style.setProperty(name, `${width}px`);
    const pane = ref.current?.parentElement;
    if (!pane) return;
    const observer = new ResizeObserver(() => setNow(Math.round(pane.getBoundingClientRect().width)));
    observer.observe(pane);
    return () => observer.disconnect();
  }, [name]);

  const width = () => ref.current?.parentElement?.getBoundingClientRect().width ?? min;
  const resize = (to: number) =>
    root.setProperty(name, `${Math.round(Math.min(max(), Math.max(min, to)))}px`);
  const keepWidth = () => keep(cookie(name), String(parseFloat(root.getPropertyValue(name))));
  const end = (e: PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    drag.current = null;
    keepWidth();
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
  };
  const keys = (e: KeyboardEvent) => {
    const step = steps[e.key];
    const to =
      step !== undefined ? width() + sign * step * (e.shiftKey ? 4 : 1) : { Home: min, End: max() }[e.key];
    if (to === undefined) return;
    e.preventDefault();
    resize(to);
    keepWidth();
  };

  return (
    // biome-ignore lint/a11y/useSemanticElements: a focusable separator is the ARIA pattern; <hr> takes no keys.
    <div
      ref={ref}
      className="resize-handle"
      data-side={side}
      role="separator"
      aria-label={label}
      aria-orientation="vertical"
      aria-valuemin={min}
      aria-valuemax={Math.round(max())}
      aria-valuenow={now}
      tabIndex={0}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        e.currentTarget.focus();
        e.currentTarget.setPointerCapture(e.pointerId);
        drag.current = { x: e.clientX, width: width() };
      }}
      onPointerMove={(e) => drag.current && resize(drag.current.width + sign * (e.clientX - drag.current.x))}
      onPointerUp={end}
      onPointerCancel={end}
      onLostPointerCapture={end}
      onKeyDown={keys}
    />
  );
}
