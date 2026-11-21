import { useEffect, useState } from 'react';
import { api, type Failure } from '../api';
import { useCard } from '../components/Corner';
import './pageframe.css';

/** An HTML page shows as it looks. */
export const isPage = (name: string) => /\.html?$/i.test(name);

/**
 * The page at `path` as it looks, its own scripts running, sealed: the frame has no origin, so it can't reach the
 * window, Gathr's API or anything of this PC's but its own folder, which the service's view of it serves.
 */
export function PageFrame({ chat, path, name }: { chat: string; path: string; name: string }) {
  const [address, setAddress] = useState<string>();
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
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
  return address ? <iframe className="pageframe" src={address} title={name} sandbox="allow-scripts" /> : null;
}
