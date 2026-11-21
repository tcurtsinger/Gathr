import { useEffect, useRef, useState } from 'react';
import { api, type Failure } from '../api';
import { useCard } from '../components/Corner';
import { FIND, KINDS, type Kind, type Part, WORDS } from './note';
import './pageframe.css';

/** An HTML page shows as it looks. */
export const isPage = (name: string) => /\.html?$/i.test(name);

/** A pin the page draws: the note's number, the part it is on, and whether it is settled. */
export interface Pin extends Part {
  n: number;
  settled: boolean;
}

/** Notes on a design's page in a chat: Comment on or off, the pins on it, and what a click on a part does. */
export interface Notes {
  on: boolean;
  pins: Pin[];
  /** A part the person clicked, with where it sits in the window. */
  onPart: (part: Part, at: DOMRect) => void;
}

/**
 * What the page says was clicked, when it is that and only that: the page is an agent's, so each word is cut short and
 * is only ever shown as text.
 */
function clicked(data: unknown): { part: Part; rect: number[] } | undefined {
  if (typeof data !== 'object' || data === null) return undefined;
  const d = data as Record<string, unknown>;
  const r = d.rect as Record<string, unknown> | undefined;
  const rect = [r?.x, r?.y, r?.w, r?.h];
  const fits =
    d.gathr === 'clicked' &&
    KINDS.includes(d.kind as Kind) &&
    typeof d.words === 'string' &&
    typeof d.find === 'string' &&
    rect.every((n) => typeof n === 'number' && Number.isFinite(n) && Math.abs(n) < 100_000);
  if (!fits) return undefined;
  const part = {
    kind: d.kind as Kind,
    words: (d.words as string).slice(0, WORDS),
    find: (d.find as string).slice(0, FIND),
  };
  return { part, rect: rect as number[] };
}

/**
 * The page at `path` as it looks, its own scripts running, sealed: the frame has no origin, so it can't reach the
 * window, Gathr's API or anything of this PC's but its own folder, which the service's view of it serves. Each new
 * `redraw` loads it again in place. Without a chat, the page is one in Designs, for the Designs screen. With `notes`
 * (a design's tab in a chat), the service serves the page with its notes script, which can only post to this window:
 * the window hears only this frame, and only a part clicked while Comment is on.
 */
export function PageFrame({
  chat,
  path,
  name,
  redraw = 0,
  notes,
}: {
  chat?: string;
  path: string;
  name: string;
  redraw?: number;
  notes?: Notes;
}) {
  const [address, setAddress] = useState<string>();
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  const frame = useRef<HTMLIFrameElement>(null);
  const drawn = useRef(redraw);
  const noted = !!notes;
  useEffect(() => {
    let live = true;
    setAddress(undefined);
    (chat ? api.view(chat, path, noted) : api.designView(path)).then(
      (a) => live && setAddress(a),
      (f: Failure) => live && setFailure(f),
    );
    return () => {
      live = false;
    };
  }, [chat, path, noted]);
  useEffect(() => {
    if (redraw === drawn.current) return;
    drawn.current = redraw;
    // A replace, not a new `src`: the old page stays until the new one is drawn, and the frame's history keeps no
    // step of it. A sealed frame's place can still be set from outside.
    const f = frame.current;
    if (f?.src) f.contentWindow?.location.replace(f.src);
  }, [redraw]);
  const latest = useRef(notes);
  latest.current = notes;
  // The page hears Comment and its pins after each load, whenever they change, and as the pointer leaves the frame
  // (headless Edge never told the sealed frame the pointer had left it).
  const tell = () => {
    const n = latest.current;
    if (n) frame.current?.contentWindow?.postMessage({ gathr: 'notes', on: n.on, pins: n.pins }, '*');
  };
  const told = JSON.stringify([notes?.on, notes?.pins]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: told is what is told
  useEffect(tell, [told]);
  useEffect(() => {
    if (!noted) return;
    const hear = (e: MessageEvent) => {
      const f = frame.current;
      const n = latest.current;
      // Only this frame's own page, never a frame inside it or another frame, and only while Comment is on.
      if (!f || e.source !== f.contentWindow || !n?.on) return;
      const got = clicked(e.data);
      if (!got) return;
      const box = f.getBoundingClientRect();
      const [x, y, w, h] = got.rect;
      n.onPart(got.part, new DOMRect(box.left + x, box.top + y, w, h));
    };
    window.addEventListener('message', hear);
    return () => window.removeEventListener('message', hear);
  }, [noted]);
  return address ? (
    <iframe
      ref={frame}
      className="pageframe"
      src={address}
      title={name}
      sandbox="allow-scripts"
      onLoad={notes ? tell : undefined}
      onPointerLeave={notes ? tell : undefined}
    />
  ) : null;
}
