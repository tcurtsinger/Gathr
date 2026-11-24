import { ArrowLeft, ArrowRight, Globe, Lock, Plus, RotateCw, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { IconButton } from '../components/IconButton';
import { PageQuestion } from './PageQuestion';
import { onCursor, onFrame, say, selected, useBrowser } from './pages';
import './browser.css';

/** How long the tab's size settles before the page is laid out again at it. */
const SETTLE = 150;
const BLANK = 'about:blank';
/** The pointers the page box shows as a page says them; anything else, a page's own picture among them, is the arrow. */
const CURSORS = new Set(
  (
    'auto default none context-menu help pointer progress wait cell crosshair text vertical-text alias copy move ' +
    'no-drop not-allowed grab grabbing all-scroll col-resize row-resize n-resize e-resize s-resize w-resize ' +
    'ne-resize nw-resize se-resize sw-resize ew-resize ns-resize nesw-resize nwse-resize zoom-in zoom-out'
  ).split(' '),
);

/** The Browser tab: the address row, then the shown page live, used with the mouse and keys. */
export function BrowserView({ open }: { open: boolean }) {
  const { missing, pages, shown, line } = useBrowser();
  const page = pages.find((p) => p.id === shown);
  const [typed, setTyped] = useState<string>();
  const screen = useRef<HTMLDivElement>(null);
  const address = useRef<HTMLInputElement>(null);
  // A new page, once shown, has the keys in the address, to type where it goes.
  const fresh = useRef(false);
  const url = page?.url === BLANK ? '' : page?.url;
  useEffect(() => {
    if (!fresh.current || !page || url) return;
    fresh.current = false;
    address.current?.focus();
  }, [page, url]);
  if (missing)
    return (
      <div className="filetab-none">
        <Globe aria-hidden="true" />
        <span>{missing}</span>
      </div>
    );
  const secure = page?.url.startsWith('https://');
  return (
    <div className="browser">
      <div className="browser-bar">
        <IconButton aria-label="Back" disabled={!page?.back} onClick={() => say({ type: 'back' })}>
          <ArrowLeft />
        </IconButton>
        <IconButton aria-label="Forward" disabled={!page?.forward} onClick={() => say({ type: 'forward' })}>
          <ArrowRight />
        </IconButton>
        {page?.loading ? (
          <IconButton aria-label="Stop" onClick={() => say({ type: 'stop' })}>
            <X />
          </IconButton>
        ) : (
          <IconButton aria-label="Reload" disabled={!page} onClick={() => say({ type: 'reload' })}>
            <RotateCw />
          </IconButton>
        )}
        <form
          className="search-box browser-address"
          onSubmit={(e) => {
            e.preventDefault();
            if (!typed?.trim()) return;
            say({ type: 'open', address: typed });
            setTyped(undefined);
            screen.current?.focus();
          }}
        >
          {secure ? <Lock aria-hidden="true" /> : <Globe aria-hidden="true" />}
          <input
            ref={address}
            value={typed ?? url ?? ''}
            placeholder="Address"
            aria-label="Address"
            autoComplete="off"
            spellCheck={false}
            onChange={(e) => setTyped(e.target.value)}
            onFocus={(e) => e.target.select()}
            onBlur={() => setTyped(undefined)}
            onKeyDown={(e) => e.key === 'Escape' && e.currentTarget.blur()}
          />
        </form>
        <IconButton
          aria-label="New page"
          onClick={() => {
            fresh.current = true;
            say({ type: 'new' });
          }}
        >
          <Plus />
        </IconButton>
      </div>
      {line && (
        <p className="browser-line" role="status">
          {line}
        </p>
      )}
      <div className="browser-stage">
        <Screen open={open} page={!!page} stopped={!!page?.dialog} box={screen} />
        {page?.dialog && <PageQuestion key={page.id} page={page} dialog={page.dialog} back={screen} />}
      </div>
    </div>
  );
}

/** Home's Browser tile: how many pages are open. */
export function BrowserTile({ onOpen }: { onOpen: () => void }) {
  const { pages } = useBrowser();
  return (
    <button type="button" className="home-tile" onClick={onOpen}>
      <span className="home-tile-name">
        <Globe aria-hidden="true" />
        Browser
      </span>
      <span className="home-tile-sub">
        {pages.length ? `${pages.length} ${pages.length === 1 ? 'page' : 'pages'}` : 'None yet'}
      </span>
    </button>
  );
}

const MODIFIERS = (e: { altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) =>
  (e.altKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.metaKey ? 4 : 0) | (e.shiftKey ? 8 : 0);
const BUTTONS = ['left', 'middle', 'right', 'back', 'forward'];

/**
 * The shown page's live picture at its real pixels. The page is laid out at this box's size, so a wider panel
 * is a wider page; the mouse, wheel and keys over it go to the page, and Ctrl+C and Ctrl+V copy and paste.
 * With no page open it holds the empty state, and still tells its size, so the first page opens at it. The pointer
 * over it is the page's. While the page's own dialog is open (`stopped`), nothing is sent to it.
 */
function Screen({
  open,
  page,
  stopped,
  box,
}: {
  open: boolean;
  page: boolean;
  stopped: boolean;
  box: React.RefObject<HTMLDivElement | null>;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const still = useRef(stopped);
  still.current = stopped;

  useEffect(
    () =>
      onCursor((cursor) => {
        if (box.current) box.current.style.cursor = CURSORS.has(cursor) ? cursor : 'default';
      }),
    [box],
  );

  // The tab's size and whether it shows: sent once it settles, and when it shows or hides.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    let wait: ReturnType<typeof setTimeout> | undefined;
    const tell = () => {
      const [width, height] = [el.clientWidth, el.clientHeight];
      const sized = width >= 50 && height >= 50;
      const visible = sized && open && document.visibilityState === 'visible';
      say({ type: 'view', visible, ...(sized && { width, height, ratio: devicePixelRatio }) });
    };
    const settle = () => {
      clearTimeout(wait);
      wait = setTimeout(tell, SETTLE);
    };
    tell();
    const seen = new ResizeObserver(settle);
    seen.observe(el);
    document.addEventListener('visibilitychange', tell);
    return () => {
      clearTimeout(wait);
      seen.disconnect();
      document.removeEventListener('visibilitychange', tell);
      say({ type: 'view', visible: false });
    };
  }, [open, box]);

  // Each frame drawn at its own pixels, the newest only, once a page is open to draw.
  useEffect(() => {
    const el = page ? canvas.current : null;
    const context = el?.getContext('2d');
    if (!el || !context) return;
    let asked = 0;
    let drawnAt = 0;
    return onFrame((frame) => {
      const n = ++asked;
      createImageBitmap(frame).then(
        (picture) => {
          if (n < drawnAt) return picture.close();
          drawnAt = n;
          if (el.width !== picture.width || el.height !== picture.height) {
            el.width = picture.width;
            el.height = picture.height;
            el.style.width = `${picture.width / devicePixelRatio}px`;
            el.style.height = `${picture.height / devicePixelRatio}px`;
          }
          context.drawImage(picture, 0, 0);
          picture.close();
        },
        () => {},
      );
    });
  }, [page]);

  // The wheel, as the page's own, never scrolling the panel; summed to one a frame.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    let summed: { x: number; y: number; dx: number; dy: number; modifiers: number } | null = null;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      if (still.current) return;
      const unit = e.deltaMode === 1 ? 40 : e.deltaMode === 2 ? el.clientHeight : 1;
      const { x, y } = at(el, e);
      if (summed) {
        summed.dx += e.deltaX * unit;
        summed.dy += e.deltaY * unit;
        return;
      }
      summed = { x, y, dx: e.deltaX * unit, dy: e.deltaY * unit, modifiers: MODIFIERS(e) };
      requestAnimationFrame(() => {
        if (summed) say({ type: 'wheel', ...summed });
        summed = null;
      });
    };
    el.addEventListener('wheel', wheel, { passive: false });
    return () => el.removeEventListener('wheel', wheel);
  }, [box]);

  // Presses counted into double and triple clicks, as the page expects them; moves sent once a frame.
  const clicks = useRef({ count: 0, at: 0, x: 0, y: 0, button: -1 });
  const moved = useRef<{ type: string } | null>(null);
  const mouse = (event: string, e: React.PointerEvent<HTMLDivElement>) => {
    if (e.pointerType === 'touch' || stopped) return;
    const { x, y } = at(e.currentTarget, e);
    const button = event === 'mouseMoved' ? 'none' : (BUTTONS[e.button] ?? 'none');
    const c = clicks.current;
    if (event === 'mousePressed') {
      const again =
        e.timeStamp - c.at < 500 && Math.abs(x - c.x) < 5 && Math.abs(y - c.y) < 5 && c.button === e.button;
      Object.assign(c, { count: again ? c.count + 1 : 1, at: e.timeStamp, x, y, button: e.button });
    }
    const said = {
      type: 'mouse',
      event,
      x,
      y,
      button,
      buttons: e.buttons,
      clickCount: event === 'mouseMoved' ? 0 : c.count,
      modifiers: MODIFIERS(e),
    };
    if (event !== 'mouseMoved') return say(said);
    if (!moved.current)
      requestAnimationFrame(() => {
        if (moved.current) say(moved.current);
        moved.current = null;
      });
    moved.current = said;
  };
  // Out of the box, the page hears the mouse leave, and the pointer is the arrow again.
  const left = (e: React.PointerEvent<HTMLDivElement>) => {
    e.currentTarget.style.cursor = 'default';
    if (e.pointerType === 'touch' || stopped) return;
    const out = {
      type: 'mouse',
      event: 'mouseMoved',
      x: -1,
      y: -1,
      button: 'none',
      buttons: 0,
      clickCount: 0,
      modifiers: 0,
    };
    if (moved.current) moved.current = out;
    else say(out);
  };
  const key = (event: string, e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.nativeEvent.isComposing || stopped) return;
    // Copy and paste stay the window's own, so the copy and paste events below run.
    const command = e.ctrlKey || e.metaKey;
    if (command && ['c', 'v'].includes(e.key.toLowerCase())) return;
    e.preventDefault();
    const printable = [...e.key].length === 1 && !command;
    const text = printable ? e.key : e.key === 'Enter' ? '\r' : '';
    say({
      type: 'key',
      event,
      key: e.key,
      code: e.code,
      keyCode: e.keyCode,
      text,
      location: e.location,
      repeat: e.repeat,
      modifiers: MODIFIERS(e),
    });
  };
  if (!page)
    return (
      <div className="browser-page" data-empty ref={box}>
        <div className="filetab-none">
          <Globe aria-hidden="true" />
          <strong>No pages</strong>
          <span>Type an address to open one.</span>
        </div>
      </div>
    );
  return (
    <div
      className="browser-page"
      ref={box}
      role="application"
      // biome-ignore lint/a11y/noNoninteractiveTabindex: the page shown takes the keys, as a page in a browser does
      tabIndex={0}
      aria-label="Page"
      onPointerDown={(e) => {
        e.preventDefault();
        // A stopped page takes nothing, the keys neither: they stay on its dialog.
        if (stopped) return;
        e.currentTarget.focus();
        e.currentTarget.setPointerCapture(e.pointerId);
        mouse('mousePressed', e);
      }}
      onPointerUp={(e) => mouse('mouseReleased', e)}
      onPointerMove={(e) => mouse('mouseMoved', e)}
      onPointerLeave={left}
      onContextMenu={(e) => e.preventDefault()}
      onKeyDown={(e) => key('keyDown', e)}
      onKeyUp={(e) => key('keyUp', e)}
      onCopy={(e) => {
        e.preventDefault();
        e.clipboardData.setData('text/plain', selected());
      }}
      onPaste={(e) => {
        e.preventDefault();
        const text = e.clipboardData.getData('text/plain');
        if (text && !stopped) say({ type: 'paste', text });
      }}
    >
      <canvas ref={canvas} />
    </div>
  );
}

/** Where an event is over the box, in the page's own pixels: inside its border, the box is the page's size. */
function at(el: HTMLElement, e: { clientX: number; clientY: number }) {
  const r = el.getBoundingClientRect();
  return {
    x: Math.round(e.clientX - r.left - el.clientLeft),
    y: Math.round(e.clientY - r.top - el.clientTop),
  };
}
