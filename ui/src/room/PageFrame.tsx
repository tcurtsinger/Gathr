import { useEffect, useRef, useState } from 'react';
import { api, type Failure } from '../api';
import { useCard } from '../components/Corner';
import './pageframe.css';

/** An HTML page shows as it looks. */
export const isPage = (name: string) => /\.html?$/i.test(name);

/**
 * The page at `path` as it looks, its own scripts running, sealed: the frame has no origin, so it can't reach the
 * window, Gathr's API or anything of this PC's but its own folder, which the service's view of it serves. Each new
 * `redraw` loads it again in place.
 */
export function PageFrame({
  chat,
  path,
  name,
  redraw = 0,
}: {
  chat: string;
  path: string;
  name: string;
  redraw?: number;
}) {
  const [address, setAddress] = useState<string>();
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  const frame = useRef<HTMLIFrameElement>(null);
  const drawn = useRef(redraw);
  useEffect(() => {
    let live = true;
    setAddress(undefined);
    api.view(chat, path).then(
      (a) => live && setAddress(a),
      (f: Failure) => live && setFailure(f),
    );
    return () => {
      live = false;
    };
  }, [chat, path]);
  useEffect(() => {
    if (redraw === drawn.current) return;
    drawn.current = redraw;
    // A replace, not a new `src`: the old page stays until the new one is drawn, and the frame's history keeps no
    // step of it. A sealed frame's place can still be set from outside.
    const f = frame.current;
    if (f?.src) f.contentWindow?.location.replace(f.src);
  }, [redraw]);
  return address ? (
    <iframe ref={frame} className="pageframe" src={address} title={name} sandbox="allow-scripts" />
  ) : null;
}
