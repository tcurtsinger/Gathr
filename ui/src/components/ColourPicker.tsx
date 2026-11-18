import {
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
  type RefObject,
  useEffect,
  useRef,
  useState,
} from 'react';
import { cleanHex, type Hsv, hexHsv, hsvHex, keepHue } from './colour';
import './colour-picker.css';

interface Props {
  /** The popover's id: the control that opens it names it with `popoverTarget`. */
  id: string;
  label: string;
  /** What it opens under, left-aligned. */
  anchor: RefObject<HTMLElement | null>;
  /** #RRGGBB. */
  value: string;
  onChange: (hex: string) => void;
  onToggle?: (open: boolean) => void;
}

const clamp = (n: number, max = 1) => Math.max(0, Math.min(max, n));
const HUE_MAX = 359;

/**
 * Any colour (0074): a square for saturation and brightness, a hue bar, the colour now and its hex. A popover, so
 * Escape or a click outside closes it and not the dialog it sits in. Each step is sent as it happens.
 */
export function ColourPicker({ id, label, anchor, value, onChange, onToggle }: Props) {
  const [hsv, setHsv] = useState<Hsv>(() => hexHsv(value));
  const [seen, setSeen] = useState(value);
  const [typed, setTyped] = useState(value.slice(1));
  // A colour from elsewhere (a swatch, a save) moves the thumbs; a grey keeps the hue bar where it was.
  if (value !== seen) {
    setSeen(value);
    if (value !== hsvHex(hsv)) setHsv(keepHue(hsv, hexHsv(value)));
    if (value !== cleanHex(typed)) setTyped(value.slice(1));
  }
  const [h, s, v] = hsv;

  const pick = (next: Hsv) => {
    setHsv(next);
    onChange(hsvHex(next));
  };
  const take = (hex: string) => {
    setHsv(keepHue(hsv, hexHsv(hex)));
    onChange(hex);
  };

  /** A press or a drag in the square or along the bar, as fractions of its box. */
  const track = (to: (x: number, y: number) => void) => (e: PointerEvent<HTMLDivElement>) => {
    const box = e.currentTarget;
    if (e.type === 'pointerdown') {
      e.preventDefault();
      box.setPointerCapture(e.pointerId);
      box.focus();
    } else if (!box.hasPointerCapture(e.pointerId)) return;
    const r = box.getBoundingClientRect();
    to(clamp((e.clientX - r.left) / r.width), clamp((e.clientY - r.top) / r.height));
  };
  const onSquare = track((x, y) => pick([h, x, 1 - y]));
  const onHue = track((x) => pick([x * HUE_MAX, s, v]));

  const squareKeys = (e: KeyboardEvent) => {
    const d = e.shiftKey ? 0.1 : 0.01;
    const by = { ArrowRight: [d, 0], ArrowLeft: [-d, 0], ArrowUp: [0, d], ArrowDown: [0, -d] }[e.key];
    if (!by) return;
    e.preventDefault();
    pick([h, clamp(s + by[0]), clamp(v + by[1])]);
  };
  const hueKeys = (e: KeyboardEvent) => {
    const d = e.shiftKey ? 10 : 1;
    const to = {
      ArrowRight: h + d,
      ArrowUp: h + d,
      ArrowLeft: h - d,
      ArrowDown: h - d,
      Home: 0,
      End: HUE_MAX,
    }[e.key];
    if (to === undefined) return;
    e.preventDefault();
    pick([clamp(Math.round(to), HUE_MAX), s, v]);
  };

  /** A hex is taken as it is typed once it has six digits, and three only when it is done (Enter or leaving it). */
  const type = (text: string) => {
    setTyped(text);
    const hex = cleanHex(text);
    if (hex && text.replace('#', '').length === 6 && hex !== value) take(hex);
  };
  const done = () => {
    const hex = cleanHex(typed);
    if (hex && hex !== value) take(hex);
    setTyped((hex ?? value).slice(1));
  };

  // Placed as it opens, and again as what it opens under scrolls.
  const panel = useRef<HTMLFieldSetElement>(null);
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!open) return;
    const follow = () => panel.current && place(panel.current, anchor.current);
    addEventListener('scroll', follow, true);
    return () => removeEventListener('scroll', follow, true);
  }, [open, anchor]);

  return (
    <fieldset
      ref={panel}
      id={id}
      popover="auto"
      aria-label={label}
      className="colour-picker"
      onToggle={(e) => {
        const shown = e.newState === 'open';
        if (shown) place(e.currentTarget, anchor.current, true);
        setOpen(shown);
        onToggle?.(shown);
      }}
    >
      <div
        className="colour-square"
        role="slider"
        tabIndex={0}
        aria-label="Colour"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(s * 100)}
        aria-valuetext={`Saturation ${Math.round(s * 100)}%, brightness ${Math.round(v * 100)}%`}
        style={{ '--hue': h } as CSSProperties}
        onPointerDown={onSquare}
        onPointerMove={onSquare}
        onKeyDown={squareKeys}
      >
        <i style={{ left: `${s * 100}%`, top: `${(1 - v) * 100}%`, background: value }} />
      </div>
      <div
        className="colour-hue"
        role="slider"
        tabIndex={0}
        aria-label="Hue"
        aria-valuemin={0}
        aria-valuemax={HUE_MAX}
        aria-valuenow={Math.round(h)}
        onPointerDown={onHue}
        onPointerMove={onHue}
        onKeyDown={hueKeys}
      >
        <i style={{ left: `${(h / HUE_MAX) * 100}%`, background: `hsl(${h} 100% 50%)` }} />
      </div>
      <div className="colour-hex">
        <span className="colour-now" style={{ background: value }} />
        <label>
          <span aria-hidden="true">#</span>
          <input
            aria-label="Hex"
            value={typed}
            maxLength={7}
            spellCheck={false}
            autoComplete="off"
            onChange={(e) => type(e.target.value)}
            onBlur={done}
            onKeyDown={(e) => {
              if (e.key !== 'Enter') return;
              // Inside a form: Enter takes the hex, never Save.
              e.preventDefault();
              done();
            }}
          />
        </label>
      </div>
    </fieldset>
  );
}

/**
 * Under the anchor, so the face the colour is for stays in sight: as it opens, what holds the anchor scrolls to make
 * room below when it can. Above it only when there's still no room; kept 20 px inside the window.
 */
function place(panel: HTMLElement, anchor: HTMLElement | null, opening = false) {
  if (!anchor) return;
  const short = () =>
    Math.ceil(anchor.getBoundingClientRect().bottom + 6 + panel.offsetHeight + 20 - innerHeight);
  if (opening && short() > 0) scroller(anchor)?.scrollBy({ top: short(), behavior: 'instant' });
  const at = anchor.getBoundingClientRect();
  const below = short() <= 0;
  panel.style.left = `${Math.max(20, Math.min(at.left, innerWidth - panel.offsetWidth - 20))}px`;
  panel.style.top = `${Math.max(20, below ? at.bottom + 6 : at.top - panel.offsetHeight - 6)}px`;
}

/** The nearest box around `el` that scrolls. */
function scroller(el: HTMLElement): HTMLElement | null {
  for (let box = el.parentElement; box; box = box.parentElement) {
    const { overflowY } = getComputedStyle(box);
    if ((overflowY === 'auto' || overflowY === 'scroll') && box.scrollHeight > box.clientHeight) return box;
  }
  return null;
}
