import { createContext, useEffect, useState } from 'react';
import { duration } from './fold';

/**
 * The timeline's components drawn in the agent's panel (0038): only there does a member's message say the task it
 * worked on (the owner's call of 2026-11-18).
 */
export const InPanel = createContext(false);

/** How long a running turn has run, redrawn each second: "4m 02s", after `before`; nothing under a second. */
export function LiveTime({ since, before = '' }: { since: number; before?: string }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const tick = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(tick);
  }, []);
  const took = duration(now - since, 'second');
  return took ? `${before}${took}` : null;
}
