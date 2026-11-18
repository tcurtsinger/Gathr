import { type RefObject, useCallback, useEffect, useLayoutEffect, useRef } from 'react';
import { marks } from './mention';

interface Props {
  text: string;
  /** Who can be named: each `@Name` of one of them is blue. */
  names: readonly string[];
  box: RefObject<HTMLTextAreaElement | null>;
}

/** Every box's blue names, one highlight (CSS Custom Highlight API); none where the browser has no such API. */
const blue = typeof Highlight === 'function' && CSS.highlights ? new Highlight() : undefined;
if (blue) CSS.highlights.set('composer-mention', blue);

/**
 * The message box's text drawn again under its see-through textarea, each `@Name` blue: the same font, padding and
 * wrap, the textarea's width without its scroll bar, and its scroll. The text stays one text node and the names are
 * coloured by a highlight, never spans: a span starts a new run of text, and the letters after it land up to 1/16px
 * from where the textarea draws them, which shows as a changed edge on a letter.
 */
export function NameLayer({ text, names, box }: Props) {
  const layer = useRef<HTMLDivElement>(null);
  const ranges = useRef<Range[]>([]);
  const follow = useCallback(() => {
    const area = box.current;
    const el = layer.current;
    if (!area || !el) return;
    el.style.width = `${area.clientWidth}px`;
    el.style.height = `${area.clientHeight}px`;
    el.scrollTop = area.scrollTop;
  }, [box]);
  // Each draw: the text may have grown the box, brought its scroll bar, or moved its scroll; the names move with it.
  useLayoutEffect(() => {
    follow();
    const node = layer.current?.firstChild;
    if (!blue) return;
    for (const r of ranges.current) blue.delete(r);
    ranges.current = [];
    if (!(node instanceof Text)) return;
    for (const part of marks(text, names)) {
      if (!part.name) continue;
      const r = new Range();
      r.setStart(node, part.at);
      r.setEnd(node, part.at + part.text.length);
      ranges.current.push(r);
      blue.add(r);
    }
  });
  useEffect(() => {
    const area = box.current;
    if (!area) return;
    const resized = new ResizeObserver(follow);
    resized.observe(area);
    area.addEventListener('scroll', follow);
    return () => {
      resized.disconnect();
      area.removeEventListener('scroll', follow);
      for (const r of ranges.current) blue?.delete(r);
    };
  }, [box, follow]);
  return (
    <div ref={layer} className="composer-mirror" aria-hidden="true">
      {`${text} `}
    </div>
  );
}
